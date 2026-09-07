//! Unit-test-only fault harness. Child exits bypass Rust destructors and release
//! OS handles exactly as process interruption does; no production env switches.
use super::*;
use arrow_array::RecordBatch;
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

const COPYBOOK: &str = include_str!("../tests/fixtures/sample_fixed.cpy");
const INPUT: &[u8] = include_bytes!("../tests/fixtures/sample_fixed.bin");

pub(super) struct FaultConfig {
    target: String,
    after_bytes: Option<u64>,
    crash: bool,
    fired: bool,
}
fn label(point: Point) -> String {
    format!("{:?}/{:?}/{:?}", point.artifact, point.stage, point.index)
}
impl FaultConfig {
    pub(super) fn hit(&mut self, point: Point, written: Option<u64>) -> io::Result<()> {
        if !self.fired && self.target == label(point) && self.after_bytes == written {
            self.fired = true;
            if self.crash {
                std::process::exit(86);
            }
            return Err(io::Error::other("injected M4 I/O failure"));
        }
        Ok(())
    }
    pub(super) fn write_limit(&self, point: Point, written: u64, requested: usize) -> usize {
        if !self.fired
            && self.target == label(point)
            && let Some(limit) = self.after_bytes
        {
            return requested.min(usize::try_from(limit.saturating_sub(written)).unwrap());
        }
        requested
    }
}

struct TestDir(PathBuf);
impl TestDir {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/m4-fault-tests");
        fs::create_dir_all(&base).unwrap();
        loop {
            let path = base.join(format!(
                "{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&path) {
                Ok(()) => {
                    fs::write(path.join("input.bin"), INPUT).unwrap();
                    return Self(path);
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("create test directory: {error}"),
            }
        }
    }
    fn input(&self) -> PathBuf {
        self.0.join("input.bin")
    }
    fn output(&self) -> PathBuf {
        self.0.join("output")
    }
}
impl Drop for TestDir {
    fn drop(&mut self) {
        let base =
            fs::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("target/m4-fault-tests"))
                .unwrap();
        let path = fs::canonicalize(&self.0).unwrap();
        assert_eq!(path.parent(), Some(base.as_path()));
        fs::remove_dir_all(path).unwrap();
    }
}

fn layout() -> CompiledCopybook {
    crate::parse_and_compile_copybook(COPYBOOK).unwrap()
}
fn run(temp: &TestDir, batch: usize, mode: RecoveryMode) {
    convert_parts(&layout(), &temp.input(), &temp.output(), batch, mode).unwrap();
}
fn command(temp: &TestDir, batch: usize, mode: RecoveryMode) -> Command {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", "recovery::tests::fault_child", "--nocapture"])
        .env("M4_TEST_CHILD", "fault")
        .env("M4_TEST_INPUT", temp.input())
        .env("M4_TEST_OUTPUT", temp.output())
        .env("M4_TEST_BATCH", batch.to_string())
        .env(
            "M4_TEST_RESUME",
            if mode == RecoveryMode::Resume {
                "1"
            } else {
                "0"
            },
        );
    command
}
fn crash(temp: &TestDir, batch: usize, mode: RecoveryMode, point: Point, after_bytes: Option<u64>) {
    let output = command(temp, batch, mode)
        .env("M4_TEST_POINT", label(point))
        .env(
            "M4_TEST_AFTER_BYTES",
            after_bytes.map_or_else(String::new, |value| value.to_string()),
        )
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(86),
        "fault not reached: {} bytes={after_bytes:?}\n{}\n{}",
        label(point),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn fault_child() {
    let Ok(child_mode) = std::env::var("M4_TEST_CHILD") else {
        return;
    };
    let input = PathBuf::from(std::env::var_os("M4_TEST_INPUT").unwrap());
    let output = PathBuf::from(std::env::var_os("M4_TEST_OUTPUT").unwrap());
    if child_mode == "lock" {
        let _guard = lock(&output).unwrap();
        println!("M4_LOCK_READY");
        io::stdout().flush().unwrap();
        io::stdin().read_exact(&mut [0_u8; 1]).unwrap();
        return;
    }
    let batch = std::env::var("M4_TEST_BATCH").unwrap().parse().unwrap();
    let mode = if std::env::var("M4_TEST_RESUME").unwrap() == "1" {
        RecoveryMode::Resume
    } else {
        RecoveryMode::Create
    };
    let mut faults = Faults {
        config: Some(FaultConfig {
            target: std::env::var("M4_TEST_POINT").unwrap(),
            after_bytes: std::env::var("M4_TEST_AFTER_BYTES").unwrap().parse().ok(),
            crash: true,
            fired: false,
        }),
    };
    convert_with_faults(&layout(), &input, &output, batch, mode, &mut faults).unwrap();
    panic!("selected crash point was never reached");
}

fn batches(root: &Path) -> Vec<RecordBatch> {
    let complete: Completion = manifest::read_json(&root.join("complete.json")).unwrap();
    let mut result = Vec::new();
    for index in 0..complete.part_count {
        let file = File::open(root.join("parts").join(part_name(index, "parquet"))).unwrap();
        let reader = ParquetRecordBatchReaderBuilder::try_new(file)
            .unwrap()
            .build()
            .unwrap();
        result.extend(reader.map(Result::unwrap));
    }
    result
}
fn committed_snapshot(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let commits = root.join("commits");
    let mut result = BTreeMap::new();
    if !commits.exists() {
        return result;
    }
    for entry in fs::read_dir(&commits).unwrap() {
        let path = entry.unwrap().path();
        if let Some((index, false)) =
            parse_part_name(path.file_name().unwrap().to_str().unwrap(), "json")
        {
            let part = root.join("parts").join(part_name(index, "parquet"));
            result.insert(path.clone(), fs::read(path).unwrap());
            result.insert(part.clone(), fs::read(part).unwrap());
        }
    }
    result
}
fn finish_and_compare(temp: &TestDir, batch: usize, expected: &[RecordBatch]) {
    let prefix = committed_snapshot(&temp.output());
    run(temp, batch, RecoveryMode::Resume);
    assert_eq!(batches(&temp.output()), expected);
    for (path, bytes) in prefix {
        assert_eq!(fs::read(path).unwrap(), bytes);
    }
    let snapshot = committed_snapshot(&temp.output());
    run(temp, batch, RecoveryMode::Resume);
    assert_eq!(committed_snapshot(&temp.output()), snapshot);
    for dir in [
        &temp.output(),
        &temp.output().join("parts"),
        &temp.output().join("commits"),
    ] {
        assert!(fs::read_dir(dir).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".tmp")
        }));
    }
}

fn cases(reference: &Path, batch: usize) -> Vec<(Point, Option<u64>)> {
    let mut cases = vec![(
        Point::new(Artifact::Bootstrap, Stage::AfterCreate, None),
        None,
    )];
    for (artifact, name) in [
        (Artifact::Manifest, "manifest.json"),
        (Artifact::Completion, "complete.json"),
    ] {
        for stage in [
            Stage::BeforeStaging,
            Stage::BeforeSync,
            Stage::AfterSync,
            Stage::BeforePublish,
            Stage::AfterPublish,
        ] {
            cases.push((Point::new(artifact, stage, None), None));
        }
        let length = fs::metadata(reference.join(name)).unwrap().len();
        for at in [1, length / 2, length - 1] {
            cases.push((Point::new(artifact, Stage::Write, None), Some(at)));
        }
    }
    for index in 0..3_usize.div_ceil(batch) as u64 {
        for stage in [
            Stage::BeforeStaging,
            Stage::BeforeFinish,
            Stage::AfterFinish,
            Stage::BeforeSync,
            Stage::AfterSync,
            Stage::BeforePublish,
            Stage::AfterPublish,
            Stage::BeforeNext,
        ] {
            cases.push((Point::new(Artifact::Part, stage, Some(index)), None));
        }
        for stage in [
            Stage::BeforeStaging,
            Stage::BeforeSync,
            Stage::AfterSync,
            Stage::BeforePublish,
            Stage::AfterPublish,
        ] {
            cases.push((Point::new(Artifact::Receipt, stage, Some(index)), None));
        }
        for (artifact, dir, extension) in [
            (Artifact::Part, "parts", "parquet"),
            (Artifact::Receipt, "commits", "json"),
        ] {
            let length = fs::metadata(reference.join(dir).join(part_name(index, extension)))
                .unwrap()
                .len();
            for at in [1, length / 2, length - 1] {
                cases.push((Point::new(artifact, Stage::Write, Some(index)), Some(at)));
            }
        }
    }
    cases
}

#[test]
fn process_interruption_matrix_converges_and_preserves_every_committed_part() {
    for batch in [1, 2] {
        let reference = TestDir::new();
        run(&reference, batch, RecoveryMode::Create);
        let expected = batches(&reference.output());
        for (point, at) in cases(&reference.output(), batch) {
            let temp = TestDir::new();
            crash(&temp, batch, RecoveryMode::Create, point, at);
            finish_and_compare(&temp, batch, &expected);
        }
    }
}

#[test]
fn returned_io_failure_matrix_converges_without_rolling_back_commits() {
    for batch in [1, 2] {
        let reference = TestDir::new();
        run(&reference, batch, RecoveryMode::Create);
        let expected = batches(&reference.output());
        for (point, at) in cases(&reference.output(), batch) {
            let temp = TestDir::new();
            let mut faults = Faults {
                config: Some(FaultConfig {
                    target: label(point),
                    after_bytes: at,
                    crash: false,
                    fired: false,
                }),
            };
            let error = convert_with_faults(
                &layout(),
                &temp.input(),
                &temp.output(),
                batch,
                RecoveryMode::Create,
                &mut faults,
            )
            .unwrap_err();
            assert!(faults.config.as_ref().unwrap().fired, "{error}");
            assert!(
                error.to_string().contains("injected M4 I/O failure"),
                "{error}"
            );
            finish_and_compare(&temp, batch, &expected);
        }
    }
}

#[test]
fn recovery_can_itself_be_interrupted_during_orphan_and_staging_cleanup() {
    let reference = TestDir::new();
    run(&reference, 1, RecoveryMode::Create);
    let expected = batches(&reference.output());
    for first_stage in [Stage::AfterPublish, Stage::AfterFinish] {
        for cleanup_stage in [Stage::BeforeRemove, Stage::AfterRemove] {
            let temp = TestDir::new();
            crash(
                &temp,
                1,
                RecoveryMode::Create,
                Point::new(Artifact::Part, first_stage, Some(1)),
                None,
            );
            let prefix = committed_snapshot(&temp.output());
            crash(
                &temp,
                1,
                RecoveryMode::Resume,
                Point::new(Artifact::Cleanup, cleanup_stage, Some(1)),
                None,
            );
            crash(
                &temp,
                1,
                RecoveryMode::Resume,
                Point::new(Artifact::Receipt, Stage::BeforePublish, Some(1)),
                None,
            );
            finish_and_compare(&temp, 1, &expected);
            for (path, bytes) in prefix {
                assert_eq!(fs::read(path).unwrap(), bytes);
            }
        }
    }
}

#[test]
fn empty_input_crashes_preserve_schema_and_complete_without_looping() {
    for (artifact, stage) in [
        (Artifact::Part, Stage::BeforePublish),
        (Artifact::Receipt, Stage::AfterPublish),
        (Artifact::Completion, Stage::AfterPublish),
    ] {
        let temp = TestDir::new();
        fs::write(temp.input(), []).unwrap();
        crash(
            &temp,
            2,
            RecoveryMode::Create,
            Point::new(
                artifact,
                stage,
                if artifact == Artifact::Completion {
                    None
                } else {
                    Some(0)
                },
            ),
            None,
        );
        run(&temp, 2, RecoveryMode::Resume);
        let reader = ParquetRecordBatchReaderBuilder::try_new(
            File::open(temp.output().join("parts").join(part_name(0, "parquet"))).unwrap(),
        )
        .unwrap();
        assert_eq!(reader.schema().as_ref(), &layout().arrow_schema);
        assert_eq!(reader.metadata().num_row_groups(), 0);
        assert_eq!(reader.metadata().file_metadata().num_rows(), 0);
        assert_eq!(
            fs::read_dir(temp.output().join("commits")).unwrap().count(),
            1
        );
    }
}

#[test]
fn os_lock_rejects_second_process_and_is_released_by_forced_termination() {
    let temp = TestDir::new();
    run(&temp, 2, RecoveryMode::Create);
    let mut child = command(&temp, 2, RecoveryMode::Resume)
        .env("M4_TEST_CHILD", "lock")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    loop {
        let mut line = String::new();
        assert_ne!(
            stdout.read_line(&mut line).unwrap(),
            0,
            "lock child exited early"
        );
        if line.contains("M4_LOCK_READY") {
            break;
        }
    }
    let before = committed_snapshot(&temp.output());
    let result = convert_parts(
        &layout(),
        &temp.input(),
        &temp.output(),
        2,
        RecoveryMode::Resume,
    );
    child.kill().unwrap();
    child.wait().unwrap();
    assert!(
        matches!(result, Err(RecoveryError::Busy { .. })),
        "{result:?}"
    );
    run(&temp, 2, RecoveryMode::Resume);
    assert_eq!(committed_snapshot(&temp.output()), before);
}

#[test]
fn artifact_names_are_unambiguous_and_range_checked() {
    for index in [0, 1, 42, u64::MAX] {
        let name = part_name(index, "parquet");
        assert_eq!(parse_part_name(&name, "parquet"), Some((index, false)));
        assert_eq!(
            parse_part_name(&format!(".{name}.tmp"), "parquet"),
            Some((index, true))
        );
    }
    for name in [
        "part-1.parquet",
        "part-18446744073709551616.parquet",
        "../part-00000000000000000000.parquet",
        "part-00000000000000000000.parquet.tmp",
    ] {
        assert_eq!(parse_part_name(name, "parquet"), None);
    }
}

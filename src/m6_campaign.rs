//! Shared test-only campaign persistence. Never compiled into the CLI/library release.
use proptest::{
    prelude::*,
    test_runner::{Config, RngSeed, TestError, TestRunner},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Case {
    pub kind: String,
    pub files: BTreeMap<String, Vec<u8>>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct CampaignConfiguration {
    cases: u32,
    mutation_bytes_min: usize,
    mutation_bytes_max_exclusive: usize,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum FailureSource {
    Corpus,
    Generated,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct FailureArtifact {
    artifact_version: u8,
    family: String,
    source: FailureSource,
    seed: u64,
    case_number: u64,
    configuration: CampaignConfiguration,
    reviewed_commit: String,
    case: Case,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ReplayArtifact {
    Failure(FailureArtifact),
    Representative(Case),
}

impl ReplayArtifact {
    fn into_case(self) -> Case {
        match self {
            Self::Failure(artifact) => artifact.case,
            Self::Representative(case) => case,
        }
    }
}

fn configuration(default: u32) -> CampaignConfiguration {
    CampaignConfiguration {
        cases: std::env::var("M6_TEST_CASES")
            .ok()
            .map(|s| s.parse().expect("M6_TEST_CASES integer"))
            .unwrap_or(default),
        mutation_bytes_min: 1,
        mutation_bytes_max_exclusive: 33,
    }
}

fn configured_seed() -> u64 {
    std::env::var("M6_TEST_SEED")
        .ok()
        .map(|s| s.parse().expect("M6_TEST_SEED integer"))
        .unwrap_or(0x4D3643)
}

fn config(configuration: &CampaignConfiguration, seed: u64) -> Config {
    Config {
        cases: configuration.cases,
        rng_seed: RngSeed::Fixed(seed),
        failure_persistence: None,
        ..Config::default()
    }
}
pub(crate) fn directory(label: &str) -> PathBuf {
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let parent = std::env::var_os("M6_TEST_OUTPUT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/m6-campaign"));
    fs::create_dir_all(&parent).unwrap();
    let path = parent.join(format!(
        "{label}-{}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&path).unwrap();
    path.canonicalize().unwrap()
}
pub(crate) fn snapshot(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn visit(root: &Path, dir: &Path, map: &mut BTreeMap<String, Vec<u8>>) {
        for e in fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            let m = fs::symlink_metadata(&p).unwrap();
            assert!(!m.file_type().is_symlink());
            if m.is_dir() {
                visit(root, &p, map)
            } else {
                map.insert(
                    p.strip_prefix(root)
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .replace('\\', "/"),
                    fs::read(&p).unwrap(),
                );
            }
        }
    }
    let mut map = BTreeMap::new();
    visit(root, root, &mut map);
    map
}
pub(crate) fn restore(case: &Case, root: &Path) {
    assert!(case.files.len() <= 32);
    for (name, bytes) in &case.files {
        let relative = Path::new(name);
        assert!(
            relative
                .components()
                .all(|c| matches!(c, std::path::Component::Normal(_)))
        );
        assert!(!name.contains(':') && bytes.len() <= 3 * 1024 * 1024);
        let path = root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        use std::io::Write;
        fs::File::create_new(path)
            .unwrap()
            .write_all(bytes)
            .unwrap();
    }
}
pub(crate) fn cleanup(root: &Path) {
    assert!(root.is_absolute() && root.file_name().unwrap().to_string_lossy().contains('-'));
    assert_eq!(root.canonicalize().unwrap(), root);
    fn no_links(p: &Path) {
        let m = fs::symlink_metadata(p).unwrap();
        assert!(!m.file_type().is_symlink());
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            assert_eq!(m.file_attributes() & 0x400, 0);
        }
        if m.is_dir() {
            for e in fs::read_dir(p).unwrap() {
                no_links(&e.unwrap().path());
            }
        }
    }
    no_links(root);
    fs::remove_dir_all(root).unwrap();
}

fn reviewed_commit() -> String {
    fn valid(commit: &str) -> bool {
        commit.len() == 40 && commit.bytes().all(|byte| byte.is_ascii_hexdigit())
    }
    if let Ok(commit) = std::env::var("M6_TEST_COMMIT")
        && valid(&commit)
    {
        return commit;
    }
    std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|commit| commit.trim().to_owned())
        .filter(|commit| valid(commit))
        .unwrap_or_else(|| "unavailable".into())
}

fn persist_failure(
    evidence: &Path,
    label: &str,
    source: FailureSource,
    seed: u64,
    case_number: u64,
    configuration: &CampaignConfiguration,
    case: Case,
) -> PathBuf {
    let path = evidence.join(format!("failure-{source:?}-{case_number}.json").to_lowercase());
    let artifact = FailureArtifact {
        artifact_version: 1,
        family: label.into(),
        source,
        seed,
        case_number,
        configuration: configuration.clone(),
        reviewed_commit: reviewed_commit(),
        case,
    };
    serde_json::to_writer(fs::File::create_new(&path).unwrap(), &artifact).unwrap();
    path
}

fn exercise(label: &str, case: Case, check: &impl Fn(&Case, &Path)) -> Result<(), TestCaseError> {
    assert_eq!(case.kind, label);
    let root = directory("case");
    restore(&case, &root);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| check(&case, &root)));
    if result.is_err() {
        return Err(TestCaseError::fail("campaign oracle failed"));
    }
    cleanup(&root);
    Ok(())
}

pub(crate) fn run(label: &str, build: impl Fn(&[u8]) -> Case, check: impl Fn(&Case, &Path)) {
    let evidence = directory(label);
    if let Some(path) = std::env::var_os("M6_TEST_REPLAY").filter(|path| !path.is_empty()) {
        let file = fs::File::open(path).unwrap();
        assert!(file.metadata().unwrap().len() <= 16 * 1024 * 1024);
        let case: ReplayArtifact = serde_json::from_reader(file).unwrap();
        exercise(label, case.into_case(), &check).unwrap();
        println!("M6_REPLAY_PASS {label}");
        return;
    }
    let configuration = configuration(8);
    let seed = configured_seed();
    let mut runner = TestRunner::new(config(&configuration, seed));
    // Deterministic seed corpus is replayed on every run before generated cases.
    let mut corpus: Vec<Vec<u8>> =
        serde_json::from_str(include_str!("../tests/fixtures/m6/mutations.json")).unwrap();
    if std::env::var_os("M6_HARNESS_GENERATED_ONLY").is_some() {
        corpus.clear();
    }
    // Keep one small concrete case on success too, so the replay gate is executable.
    let representative = build(corpus.first().map_or(&[0], Vec::as_slice));
    serde_json::to_writer(
        fs::File::create_new(evidence.join("replay.json")).unwrap(),
        &representative,
    )
    .unwrap();
    for (index, mutation) in corpus.into_iter().enumerate() {
        let case = build(&mutation);
        if exercise(label, case.clone(), &check).is_err() {
            let path = persist_failure(
                &evidence,
                label,
                FailureSource::Corpus,
                seed,
                index as u64 + 1,
                &configuration,
                case,
            );
            panic!("concrete corpus failure saved in {}", path.display());
        }
    }
    let count = std::cell::Cell::new(0_u64);
    let result = runner.run(&prop::collection::vec(any::<u8>(), 1..33), |bytes| {
        count.set(count.get() + 1);
        println!("M6_CASE {label} {}", count.get());
        exercise(label, build(&bytes), &check)
    });
    println!(
        "M6_CAMPAIGN {label} cases={} seed={:?} result={}",
        count.get(),
        runner.config().rng_seed,
        result.is_ok()
    );
    match result {
        Ok(()) => println!("M6_REPLAY_CASE {}", evidence.join("replay.json").display()),
        Err(TestError::Fail(reason, minimal)) => {
            let path = persist_failure(
                &evidence,
                label,
                FailureSource::Generated,
                seed,
                count.get(),
                &configuration,
                build(&minimal),
            );
            panic!(
                "{reason}; minimized concrete failure saved in {}",
                path.display()
            );
        }
        Err(TestError::Abort(reason)) => panic!("campaign aborted without a case: {reason}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::{Command, Output};

    const CHILD_TEST: &str = "m6_campaign::tests::controlled_oracle_child";
    const REVIEWED_COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

    #[test]
    fn controlled_oracle_child() {
        let Ok(mode) = std::env::var("M6_HARNESS_SELF_TEST") else {
            return;
        };
        run(
            "harness-self-test",
            |bytes| Case {
                kind: "harness-self-test".into(),
                files: BTreeMap::from([("mutation.bin".into(), bytes.to_vec())]),
            },
            |case, _| {
                assert_ne!(mode, "fail", "intentional test-only oracle failure");
                assert!(!case.files["mutation.bin"].is_empty());
            },
        );
    }

    fn child(output: &Path, mode: &str, replay: Option<&Path>) -> Output {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args(["--exact", CHILD_TEST, "--nocapture", "--test-threads=1"])
            .env("M6_HARNESS_SELF_TEST", mode)
            .env("M6_TEST_OUTPUT", output)
            .env("M6_TEST_COMMIT", REVIEWED_COMMIT);
        if let Some(replay) = replay {
            // Replay must not parse or regenerate the original PRNG configuration.
            command
                .env("M6_TEST_REPLAY", replay)
                .env("M6_TEST_SEED", "not-used-by-replay")
                .env("M6_TEST_CASES", "not-used-by-replay");
        } else {
            command
                .env_remove("M6_TEST_REPLAY")
                .env("M6_HARNESS_GENERATED_ONLY", "1")
                .env("M6_TEST_SEED", "24301")
                .env("M6_TEST_CASES", "1");
        }
        command.output().unwrap()
    }

    fn find_failure(root: &Path) -> PathBuf {
        for directory in fs::read_dir(root).unwrap() {
            let directory = directory.unwrap().path();
            if !directory.is_dir() {
                continue;
            }
            for file in fs::read_dir(directory).unwrap() {
                let file = file.unwrap().path();
                if file
                    .file_name()
                    .is_some_and(|name| name.to_string_lossy().starts_with("failure-generated-"))
                {
                    return file;
                }
            }
        }
        panic!("generated failure artifact was not persisted")
    }

    #[test]
    fn artificial_generated_failure_persists_and_replays_without_rng() {
        let output = directory("harness-self-test-parent");
        let failed = child(&output, "fail", None);
        assert!(!failed.status.success(), "artificial failure became PASS");

        let path = find_failure(&output);
        let artifact: FailureArtifact =
            serde_json::from_reader(fs::File::open(&path).unwrap()).unwrap();
        assert_eq!(artifact.artifact_version, 1);
        assert_eq!(artifact.family, "harness-self-test");
        assert_eq!(artifact.source, FailureSource::Generated);
        assert_eq!(artifact.seed, 24301);
        assert!(artifact.case_number > 0);
        assert_eq!(
            artifact.configuration,
            CampaignConfiguration {
                cases: 1,
                mutation_bytes_min: 1,
                mutation_bytes_max_exclusive: 33,
            }
        );
        assert_eq!(artifact.reviewed_commit, REVIEWED_COMMIT);
        assert_eq!(artifact.case.kind, "harness-self-test");
        assert!(!artifact.case.files["mutation.bin"].is_empty());

        let replayed_failure = child(&output, "fail", Some(&path));
        assert!(
            !replayed_failure.status.success(),
            "replayed failing oracle became PASS"
        );
        let replayed_success = child(&output, "success", Some(&path));
        assert!(replayed_success.status.success(), "{replayed_success:?}");
        assert!(
            String::from_utf8_lossy(&replayed_success.stdout)
                .contains("M6_REPLAY_PASS harness-self-test")
        );
        cleanup(&output);
    }
}

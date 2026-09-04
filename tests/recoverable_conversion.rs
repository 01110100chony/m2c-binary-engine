//! Public M4 acceptance tests. Expected values come from the annotated binary
//! fixture, independently of the implementation's parser and decoder.

use std::collections::{BTreeMap, HashMap};
use std::error::Error;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use arrow_array::{Array, ArrayRef, Decimal128Array, Int64Array, RecordBatch, StringArray};
use arrow_schema::{DataType, Field, Schema};
use m2c_pipeline::{
    CompiledCopybook, ConversionError, DecodeErrorKind, RecoveryError, RecoveryMode, convert_file,
    convert_parts, parse_and_compile_copybook,
};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::basic::Compression;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const COPYBOOK: &str = include_str!("fixtures/sample_fixed.cpy");
const BINARY: &[u8] = include_bytes!("fixtures/sample_fixed.bin");
const INPUT_SHA256: &str = "bc5083614c9c50322a78ea30b909fabb28d63a22f0d4bda87f77dfd49e47fb73";

struct TestDir {
    root: PathBuf,
    path: PathBuf,
}

impl TestDir {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/m4-integration-tests");
        fs::create_dir_all(&root).unwrap();
        let root = root.canonicalize().unwrap();
        loop {
            let path = root.join(format!(
                "{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self { root, path },
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("test directory: {error}"),
            }
        }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }

    fn input(&self, bytes: &[u8]) -> PathBuf {
        let path = self.path("entrada com espaços.bin");
        fs::write(&path, bytes).unwrap();
        path
    }

    fn command(&self, input: &Path, output: &Path, rows: &str) -> Command {
        let copybook = self.path("layout.cpy");
        fs::write(&copybook, COPYBOOK).unwrap();
        let mut command = Command::new(env!("CARGO_BIN_EXE_m2c-pipeline"));
        command
            .arg("convert-parts")
            .arg("--copybook")
            .arg(copybook)
            .arg("--input")
            .arg(input)
            .arg("--output-dir")
            .arg(output)
            .arg("--batch-records")
            .arg(rows);
        command
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        // Only this instance's uniquely created directory, under the verified
        // test root, may be removed recursively. Never resolve arbitrary input.
        assert_eq!(self.path.parent(), Some(self.root.as_path()));
        assert!(self.path.canonicalize().unwrap().starts_with(&self.root));
        fs::remove_dir_all(&self.path).unwrap();
    }
}

fn expected_batch() -> RecordBatch {
    let fields = [
        ("CUSTOMER-NAME", DataType::Utf8),
        ("ACCOUNT-NUMBER", DataType::Int64),
        ("INTEREST-RATE", DataType::Decimal128(7, 2)),
        ("BALANCE-BIN", DataType::Int64),
        ("RATE-BIN", DataType::Decimal128(7, 2)),
        ("AMOUNT-PACKED", DataType::Decimal128(9, 2)),
    ]
    .into_iter()
    .map(|(name, kind)| Field::new(format!("SAMPLE-RECORD.HEADER-GROUP.{name}"), kind, false))
    .collect::<Vec<_>>();
    let columns: Vec<ArrayRef> = vec![
        Arc::new(StringArray::from(vec![
            "ALICE     ",
            "José      ",
            "\0\u{85}\n¤[]    ",
        ])),
        Arc::new(Int64Array::from(vec![42, 9999, 0])),
        Arc::new(
            Decimal128Array::from(vec![12345, 9999999, 0])
                .with_precision_and_scale(7, 2)
                .unwrap(),
        ),
        Arc::new(Int64Array::from(vec![-123, 9999, 0])),
        Arc::new(
            Decimal128Array::from(vec![123456, 9999999, 0])
                .with_precision_and_scale(7, 2)
                .unwrap(),
        ),
        Arc::new(
            Decimal128Array::from(vec![123456789, -123, 0])
                .with_precision_and_scale(9, 2)
                .unwrap(),
        ),
    ];
    RecordBatch::try_new(Arc::new(Schema::new(fields)), columns).unwrap()
}

fn part(output: &Path, index: usize) -> PathBuf {
    output.join(format!("parts/part-{index:020}.parquet"))
}

fn receipt(output: &Path, index: usize) -> PathBuf {
    output.join(format!("commits/part-{index:020}.json"))
}

fn part_tmp(output: &Path, index: usize) -> PathBuf {
    output.join(format!("parts/.part-{index:020}.parquet.tmp"))
}

fn receipt_tmp(output: &Path, index: usize) -> PathBuf {
    output.join(format!("commits/.part-{index:020}.json.tmp"))
}

fn document(path: &Path) -> Value {
    let bytes = fs::read(path).unwrap();
    assert!(bytes.len() <= 4096);
    serde_json::from_slice(&bytes).unwrap()
}

fn write_document(path: &Path, value: &Value) {
    fs::write(path, serde_json::to_vec(value).unwrap()).unwrap();
}

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn is_link(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

// Includes empty directories and links but does not follow links. This proves
// rejected resume does not clean staging, remove files or publish artifacts.
fn snapshot(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn visit(root: &Path, directory: &Path, result: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            let key = path.strip_prefix(root).unwrap().to_path_buf();
            let metadata = fs::symlink_metadata(&path).unwrap();
            let bytes = if is_link(&metadata) {
                fs::read_link(&path)
                    .unwrap()
                    .to_string_lossy()
                    .as_bytes()
                    .to_vec()
            } else if metadata.is_dir() {
                visit(root, &path, result);
                b"<directory>".to_vec()
            } else {
                fs::read(path).unwrap()
            };
            result.insert(key, bytes);
        }
    }
    let mut result = BTreeMap::new();
    visit(root, root, &mut result);
    result
}

fn reject_without_changes(
    layout: &CompiledCopybook,
    input: &Path,
    output: &Path,
    batch_records: usize,
) -> RecoveryError {
    let before = snapshot(output);
    let error = convert_parts(layout, input, output, batch_records, RecoveryMode::Resume)
        .expect_err("invalid or incompatible recovery state must be rejected");
    assert_eq!(
        snapshot(output),
        before,
        "rejected resume changed artifacts: {error}"
    );
    error
}

fn create_fixture(temp: &TestDir, name: &str, rows: usize) -> (CompiledCopybook, PathBuf, PathBuf) {
    let layout = parse_and_compile_copybook(COPYBOOK).unwrap();
    let input = temp.input(BINARY);
    let output = temp.path(name);
    convert_parts(&layout, &input, &output, rows, RecoveryMode::Create).unwrap();
    (layout, input, output)
}

fn make_last_part_orphan(output: &Path, last: usize) {
    fs::remove_file(output.join("complete.json")).unwrap();
    fs::remove_file(receipt(output, last)).unwrap();
}

fn assert_parts(output: &Path, batch_records: usize, expected: &RecordBatch) {
    let manifest = document(&output.join("manifest.json"));
    assert_eq!(manifest["format"], "m2c-m4");
    assert_eq!(manifest["version"], 1);
    assert_eq!(manifest["record_length"], 35);
    assert_eq!(manifest["batch_records"], batch_records);
    assert_eq!(
        manifest["profile"],
        "m2c-v0.1-cp037-parquet53-uncompressed-v1"
    );
    // Value's default JSON object uses sorted keys, independently of the
    // manifest DTO's field order. This checks the persisted job identifier.
    let mut identity = manifest.clone();
    identity.as_object_mut().unwrap().remove("job_id");
    assert_eq!(
        manifest["job_id"],
        hash(&serde_json::to_vec(&identity).unwrap())
    );
    assert_eq!(manifest["input_bytes"], expected.num_rows() * 35);
    assert_eq!(
        manifest["input_sha256"],
        if expected.num_rows() == 0 {
            hash(&[])
        } else {
            INPUT_SHA256.to_owned()
        }
    );
    let count = expected.num_rows().div_ceil(batch_records).max(1);
    let complete = document(&output.join("complete.json"));
    assert_eq!(complete["version"], 1);
    assert_eq!(complete["job_id"], manifest["job_id"]);
    assert_eq!(complete["part_count"], count);
    assert_eq!(complete["total_records"], expected.num_rows());
    let mut offset = 0;
    for index in 0..count {
        let rows = batch_records.min(expected.num_rows() - offset);
        let parquet = part(output, index);
        let commit = document(&receipt(output, index));
        assert_eq!(commit["version"], 1);
        assert_eq!(commit["job_id"], manifest["job_id"]);
        assert_eq!(commit["part_index"], index);
        assert_eq!(commit["start_record"], offset);
        assert_eq!(commit["record_count"], rows);
        let bytes = fs::read(&parquet).unwrap();
        assert_eq!(commit["parquet_bytes"], bytes.len());
        assert_eq!(commit["parquet_sha256"], hash(&bytes));
        let reader =
            ParquetRecordBatchReaderBuilder::try_new(File::open(&parquet).unwrap()).unwrap();
        assert_eq!(reader.schema(), &expected.schema());
        assert_eq!(reader.metadata().file_metadata().num_rows(), rows as i64);
        assert_eq!(reader.metadata().num_row_groups(), usize::from(rows > 0));
        for group in reader.metadata().row_groups() {
            for column in group.columns() {
                assert_eq!(column.compression(), Compression::UNCOMPRESSED);
            }
        }
        let start = offset;
        for batch in reader.with_batch_size(batch_records).build().unwrap() {
            let batch = batch.unwrap();
            // Parquet 53 intentionally strips top-level metadata from the
            // reader's batches. Its builder schema above verifies all persisted
            // metadata; compare fields and values here without losing that check.
            let expected_slice = expected.slice(offset, batch.num_rows());
            let expected_read = RecordBatch::try_new(
                Arc::new(Schema::new(expected.schema().fields.to_vec())),
                expected_slice.columns().to_vec(),
            )
            .unwrap();
            assert_eq!(batch, expected_read);
            for column in batch.columns() {
                column.to_data().validate_full().unwrap();
            }
            offset += batch.num_rows();
        }
        assert_eq!(offset - start, rows);
    }
    assert_eq!(offset, expected.num_rows());
    assert_eq!(fs::read_dir(output.join("parts")).unwrap().count(), count);
    assert_eq!(fs::read_dir(output.join("commits")).unwrap().count(), count);
}

fn assert_cli_success(result: Output) {
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

fn assert_cli_failure(result: Output, text: &str) {
    assert!(!result.status.success());
    let stderr = String::from_utf8(result.stderr).unwrap();
    assert!(stderr.contains(text), "expected {text:?}, got {stderr}");
    assert!(!stderr.contains("panicked"), "{stderr}");
}

#[test]
fn golden_parts_have_exact_ranges_values_schema_and_deterministic_job_identity() {
    let temp = TestDir::new();
    for rows in [1, 2] {
        let (layout, input, output) = create_fixture(&temp, &format!("batch-{rows}"), rows);
        assert_parts(&output, rows, &expected_batch());
        let other = temp.path(&format!("other-{rows}"));
        convert_parts(&layout, &input, &other, rows, RecoveryMode::Create).unwrap();
        assert_parts(&other, rows, &expected_batch());
        assert_eq!(
            document(&output.join("manifest.json")),
            document(&other.join("manifest.json"))
        );
    }
}

#[test]
fn completed_resume_is_idempotent_and_create_never_overwrites() {
    let temp = TestDir::new();
    let (layout, input, output) = create_fixture(&temp, "complete", 2);
    let before = snapshot(&output);
    for _ in 0..2 {
        convert_parts(&layout, &input, &output, 2, RecoveryMode::Resume).unwrap();
        assert_eq!(snapshot(&output), before);
    }
    assert!(convert_parts(&layout, &input, &output, 2, RecoveryMode::Create).is_err());
    assert_eq!(snapshot(&output), before);
    fs::remove_file(&input).unwrap();
    reject_without_changes(&layout, &input, &output, 2);
}

#[test]
fn cli_creates_and_resumes_parts_with_paths_containing_spaces() {
    let temp = TestDir::new();
    let input = temp.input(BINARY);
    let output = temp.path("saída em partes");
    assert_cli_success(temp.command(&input, &output, "2").output().unwrap());
    assert_parts(&output, 2, &expected_batch());
    let before = snapshot(&output);
    assert_cli_success(
        temp.command(&input, &output, "2")
            .arg("--resume")
            .output()
            .unwrap(),
    );
    assert_eq!(snapshot(&output), before);
    assert!(
        !temp
            .command(&input, &output, "2")
            .output()
            .unwrap()
            .status
            .success()
    );
    assert_eq!(snapshot(&output), before);
}

#[test]
fn empty_input_has_one_schema_only_part_and_idempotent_resume() {
    let temp = TestDir::new();
    let layout = parse_and_compile_copybook(COPYBOOK).unwrap();
    let input = temp.input(&[]);
    let output = temp.path("empty");
    convert_parts(&layout, &input, &output, 2, RecoveryMode::Create).unwrap();
    assert_parts(&output, 2, &expected_batch().slice(0, 0));
    let before = snapshot(&output);
    convert_parts(&layout, &input, &output, 2, RecoveryMode::Resume).unwrap();
    assert_eq!(snapshot(&output), before);
    fs::remove_file(output.join("complete.json")).unwrap();
    convert_parts(&layout, &input, &output, 2, RecoveryMode::Resume).unwrap();
    assert_eq!(snapshot(&output), before);
}

#[test]
fn relocated_identical_input_and_semantically_identical_copybook_can_resume() {
    let temp = TestDir::new();
    let (layout, input, output) = create_fixture(&temp, "relocated", 2);
    make_last_part_orphan(&output, 1);
    let first_part = fs::read(part(&output, 0)).unwrap();
    let relocated = temp.path("renamed.bin");
    fs::write(&relocated, BINARY).unwrap();
    fs::remove_file(input).unwrap();
    let formatted =
        parse_and_compile_copybook(&format!("      * a new comment\n{COPYBOOK}")).unwrap();
    assert_eq!(formatted.arrow_schema, layout.arrow_schema);
    assert_ne!(formatted.fields[0].span, layout.fields[0].span);
    convert_parts(&formatted, &relocated, &output, 2, RecoveryMode::Resume).unwrap();
    assert_eq!(fs::read(part(&output, 0)).unwrap(), first_part);
    assert_parts(&output, 2, &expected_batch());
    // Replacing the file with identical bytes (and a new timestamp) is allowed.
    fs::write(&relocated, BINARY).unwrap();
    convert_parts(&layout, &relocated, &output, 2, RecoveryMode::Resume).unwrap();
}

#[test]
fn changed_input_prefix_suffix_and_batch_configuration_fail_without_cleanup() {
    let temp = TestDir::new();
    let (layout, input, output) = create_fixture(&temp, "identity", 2);
    make_last_part_orphan(&output, 1);
    fs::write(part_tmp(&output, 1), b"unfinished staging").unwrap();
    for offset in [0, BINARY.len() - 1] {
        let mut bytes = BINARY.to_vec();
        bytes[offset] ^= 1;
        fs::write(&input, bytes).unwrap();
        assert!(matches!(
            reject_without_changes(&layout, &input, &output, 2),
            RecoveryError::IncompatibleResume { .. }
        ));
    }
    fs::write(&input, BINARY).unwrap();
    assert!(matches!(
        reject_without_changes(&layout, &input, &output, 1),
        RecoveryError::IncompatibleResume { .. }
    ));
}

#[test]
fn physical_filler_layout_and_arrow_schema_metadata_are_part_of_identity() {
    let temp = TestDir::new();
    let (layout, input, output) = create_fixture(&temp, "layout-identity", 2);
    make_last_part_orphan(&output, 1);
    fs::write(receipt_tmp(&output, 1), b"unfinished receipt").unwrap();
    let physical =
        parse_and_compile_copybook(&COPYBOOK.replace("FILLER PIC X(2)", "FILLER PIC 9(2)"))
            .unwrap();
    assert_eq!(physical.record_length, layout.record_length);
    assert_eq!(physical.arrow_schema, layout.arrow_schema);
    assert_ne!(physical.fields, layout.fields);
    let renamed =
        parse_and_compile_copybook(&COPYBOOK.replace("CUSTOMER-NAME", "CLIENT-NAME")).unwrap();
    let mut schema_metadata = layout.clone();
    schema_metadata
        .arrow_schema
        .metadata
        .insert("origin".into(), "changed".into());
    let mut field_metadata = layout.clone();
    let mut fields = field_metadata.arrow_schema.fields.to_vec();
    fields[0] = Arc::new(
        fields[0]
            .as_ref()
            .clone()
            .with_metadata(HashMap::from([("unit".into(), "changed".into())])),
    );
    field_metadata.arrow_schema = Schema::new(fields);
    for changed in [physical, renamed, schema_metadata, field_metadata] {
        assert!(matches!(
            reject_without_changes(&changed, &input, &output, 2),
            RecoveryError::IncompatibleResume { .. }
        ));
    }
}

fn with_metadata(mut layout: CompiledCopybook, reverse: bool) -> CompiledCopybook {
    let mut pairs = vec![
        ("alpha".to_owned(), "á".to_owned()),
        ("zeta".to_owned(), "z".to_owned()),
    ];
    if reverse {
        pairs.reverse();
    }
    let mut fields = layout.arrow_schema.fields.to_vec();
    fields[0] = Arc::new(
        fields[0]
            .as_ref()
            .clone()
            .with_metadata(pairs.iter().cloned().collect()),
    );
    layout.arrow_schema = Schema::new_with_metadata(fields, pairs.into_iter().collect());
    layout
}

#[test]
fn schema_and_field_metadata_roundtrip_and_map_order_does_not_change_identity() {
    let temp = TestDir::new();
    let layout = with_metadata(parse_and_compile_copybook(COPYBOOK).unwrap(), false);
    let equivalent = with_metadata(parse_and_compile_copybook(COPYBOOK).unwrap(), true);
    let input = temp.input(BINARY);
    let output = temp.path("metadata");
    convert_parts(&layout, &input, &output, 2, RecoveryMode::Create).unwrap();
    let expected = RecordBatch::try_new(
        Arc::new(layout.arrow_schema.clone()),
        expected_batch().columns().to_vec(),
    )
    .unwrap();
    assert_parts(&output, 2, &expected);
    let before = snapshot(&output);
    convert_parts(&equivalent, &input, &output, 2, RecoveryMode::Resume).unwrap();
    assert_eq!(snapshot(&output), before);
    let other = temp.path("metadata-equivalent");
    convert_parts(&equivalent, &input, &other, 2, RecoveryMode::Create).unwrap();
    assert_eq!(
        document(&output.join("manifest.json")),
        document(&other.join("manifest.json"))
    );
}

#[test]
fn malformed_documents_are_rejected_before_cleaning_any_staged_artifact() {
    let temp = TestDir::new();
    for target in [
        "manifest.json",
        "commits/part-00000000000000000000.json",
        "complete.json",
    ] {
        let name = target.replace(['/', '.'], "-");
        let (layout, input, output) = create_fixture(&temp, &name, 2);
        let path = output.join(target);
        let original = fs::read(&path).unwrap();
        let value: Value = serde_json::from_slice(&original).unwrap();
        fs::write(
            output.join(".complete.json.tmp"),
            b"do not clean before validation",
        )
        .unwrap();
        let mut unknown = value.clone();
        unknown["extra"] = json!(true);
        let mut missing = value.clone();
        missing.as_object_mut().unwrap().remove("job_id");
        let mut bad_hash = value.clone();
        bad_hash["job_id"] = json!("not-a-sha256");
        let mut invalid_number = value.clone();
        invalid_number["version"] = json!(-1);
        let mut overflow = value.clone();
        overflow["version"] = json!(4_294_967_296_u64);
        let mut fractional = value.clone();
        fractional["version"] = json!(1.5);
        let duplicate = format!(
            "{{\"job_id\":{},{}",
            value["job_id"],
            std::str::from_utf8(&original[1..]).unwrap()
        );
        let mut trailing = original.clone();
        trailing.extend_from_slice(b" {}");
        let invalid_cases = [
            b"{".to_vec(),
            vec![b' '; 4097],
            serde_json::to_vec(&unknown).unwrap(),
            serde_json::to_vec(&missing).unwrap(),
            serde_json::to_vec(&bad_hash).unwrap(),
            serde_json::to_vec(&invalid_number).unwrap(),
            serde_json::to_vec(&overflow).unwrap(),
            serde_json::to_vec(&fractional).unwrap(),
            duplicate.into_bytes(),
            trailing,
        ];
        for invalid in invalid_cases {
            fs::write(&path, invalid).unwrap();
            reject_without_changes(&layout, &input, &output, 2);
        }
    }
}

#[test]
fn documents_at_the_four_kibibyte_limit_remain_valid() {
    let temp = TestDir::new();
    let (layout, input, output) = create_fixture(&temp, "bounded-metadata", 2);
    for path in [
        output.join("manifest.json"),
        receipt(&output, 0),
        output.join("complete.json"),
    ] {
        let mut bytes = fs::read(&path).unwrap();
        bytes.resize(4096, b' ');
        fs::write(path, bytes).unwrap();
    }
    let before = snapshot(&output);
    convert_parts(&layout, &input, &output, 2, RecoveryMode::Resume).unwrap();
    assert_eq!(snapshot(&output), before);
}

#[test]
fn unsupported_manifest_version_and_profile_return_distinct_errors() {
    let temp = TestDir::new();
    let (layout, input, output) = create_fixture(&temp, "unsupported", 2);
    let path = output.join("manifest.json");
    let original = document(&path);
    let mut changed = original.clone();
    changed["version"] = json!(42);
    write_document(&path, &changed);
    assert!(matches!(
        reject_without_changes(&layout, &input, &output, 2),
        RecoveryError::UnsupportedVersion { version: 42 }
    ));
    changed = original;
    changed["profile"] = json!("unknown-profile");
    write_document(&path, &changed);
    assert!(matches!(
        reject_without_changes(&layout, &input, &output, 2),
        RecoveryError::UnsupportedProfile { .. }
    ));
}

#[test]
fn missing_truncated_and_same_size_corrupted_committed_parts_are_not_regenerated() {
    let temp = TestDir::new();
    let (layout, input, output) = create_fixture(&temp, "corrupt-part", 2);
    make_last_part_orphan(&output, 1);
    fs::write(part_tmp(&output, 1), b"staging must survive rejection").unwrap();
    let path = part(&output, 0);
    let original = fs::read(&path).unwrap();
    fs::remove_file(&path).unwrap();
    assert!(matches!(
        reject_without_changes(&layout, &input, &output, 2),
        RecoveryError::MissingCommittedPart { part_index: 0, .. }
    ));
    fs::write(&path, &original[..original.len() - 1]).unwrap();
    assert!(matches!(
        reject_without_changes(&layout, &input, &output, 2),
        RecoveryError::CorruptCommittedPart { part_index: 0, .. }
    ));
    let mut changed = original;
    changed[8] ^= 1;
    fs::write(&path, changed).unwrap();
    assert!(matches!(
        reject_without_changes(&layout, &input, &output, 2),
        RecoveryError::CorruptCommittedPart { part_index: 0, .. }
    ));
}

#[test]
fn committed_parquet_schema_row_count_and_footer_are_validated_beyond_hash() {
    let temp = TestDir::new();
    let (layout, input, output) = create_fixture(&temp, "parquet-validation", 2);
    let commit_path = receipt(&output, 0);
    let mut commit = document(&commit_path);
    let named_differently =
        parse_and_compile_copybook(&COPYBOOK.replace("CUSTOMER-NAME", "CLIENT-NAME")).unwrap();
    let row_input = temp.path("one-row.bin");
    fs::write(&row_input, &BINARY[..35]).unwrap();
    let two_rows_input = temp.path("two-rows.bin");
    fs::write(&two_rows_input, &BINARY[..70]).unwrap();
    let wrong_schema = temp.path("wrong-schema.parquet");
    convert_file(&named_differently, &two_rows_input, &wrong_schema, 2).unwrap();
    let wrong_rows = temp.path("wrong-rows.parquet");
    convert_file(&layout, &row_input, &wrong_rows, 2).unwrap();
    for bytes in [
        fs::read(wrong_schema).unwrap(),
        fs::read(wrong_rows).unwrap(),
        b"not a parquet footer".to_vec(),
    ] {
        let invalid_footer = bytes == b"not a parquet footer";
        fs::write(part(&output, 0), &bytes).unwrap();
        commit["parquet_bytes"] = json!(bytes.len());
        commit["parquet_sha256"] = json!(hash(&bytes));
        write_document(&commit_path, &commit);
        let error = reject_without_changes(&layout, &input, &output, 2);
        if invalid_footer {
            assert!(
                error
                    .source()
                    .unwrap()
                    .downcast_ref::<parquet::errors::ParquetError>()
                    .is_some()
            );
        }
        assert!(matches!(
            error,
            RecoveryError::CorruptCommittedPart { part_index: 0, .. }
        ));
    }
}

#[test]
fn gaps_mismatched_indices_and_premature_completion_are_rejected() {
    let temp = TestDir::new();
    let (layout, input, output) = create_fixture(&temp, "gap", 1);
    fs::remove_file(receipt(&output, 1)).unwrap();
    reject_without_changes(&layout, &input, &output, 1);
    let (layout, input, output) = create_fixture(&temp, "receipt-fields", 2);
    let path = receipt(&output, 0);
    let original = document(&path);
    for (field, invalid) in [
        ("part_index", json!(1)),
        ("start_record", json!(1)),
        ("record_count", json!(1)),
        ("job_id", json!("0".repeat(64))),
    ] {
        let mut value = original.clone();
        value[field] = invalid;
        write_document(&path, &value);
        reject_without_changes(&layout, &input, &output, 2);
    }
    let (layout, input, output) = create_fixture(&temp, "premature", 2);
    fs::remove_file(receipt(&output, 1)).unwrap();
    fs::remove_file(part(&output, 1)).unwrap();
    reject_without_changes(&layout, &input, &output, 2);
    let (layout, input, output) = create_fixture(&temp, "completion-fields", 2);
    let path = output.join("complete.json");
    let original = document(&path);
    for (field, invalid) in [
        ("part_count", json!(1)),
        ("total_records", json!(2)),
        ("job_id", json!("0".repeat(64))),
    ] {
        let mut value = original.clone();
        value[field] = invalid;
        write_document(&path, &value);
        reject_without_changes(&layout, &input, &output, 2);
    }
}

#[test]
fn next_orphan_and_recognized_temporaries_are_discarded_preserving_commits() {
    let temp = TestDir::new();
    for orphan_contents in [None, Some(b"incomplete parquet".as_slice())] {
        let name = if orphan_contents.is_some() {
            "truncated-orphan"
        } else {
            "valid-orphan"
        };
        let (layout, input, output) = create_fixture(&temp, name, 2);
        make_last_part_orphan(&output, 1);
        let committed_part = fs::read(part(&output, 0)).unwrap();
        let committed_receipt = fs::read(receipt(&output, 0)).unwrap();
        if let Some(bytes) = orphan_contents {
            fs::write(part(&output, 1), bytes).unwrap();
        }
        fs::write(part_tmp(&output, 1), b"partial parquet").unwrap();
        fs::write(receipt_tmp(&output, 1), b"{\"version\":").unwrap();
        fs::write(output.join(".complete.json.tmp"), b"partial completion").unwrap();
        convert_parts(&layout, &input, &output, 2, RecoveryMode::Resume).unwrap();
        assert_parts(&output, 2, &expected_batch());
        assert_eq!(fs::read(part(&output, 0)).unwrap(), committed_part);
        assert_eq!(fs::read(receipt(&output, 0)).unwrap(), committed_receipt);
        assert!(!part_tmp(&output, 1).exists());
        assert!(!receipt_tmp(&output, 1).exists());
        assert!(!output.join(".complete.json.tmp").exists());
    }
}

#[test]
fn all_parts_committed_without_completion_publishes_only_completion() {
    let temp = TestDir::new();
    let (layout, input, output) = create_fixture(&temp, "missing-completion", 2);
    let original_complete = fs::read(output.join("complete.json")).unwrap();
    fs::remove_file(output.join("complete.json")).unwrap();
    let before = snapshot(&output);
    convert_parts(&layout, &input, &output, 2, RecoveryMode::Resume).unwrap();
    let mut after = snapshot(&output);
    assert_eq!(
        after.remove(Path::new("complete.json")).unwrap(),
        original_complete
    );
    assert_eq!(after, before);
}

#[test]
fn unknown_namespace_entries_and_parts_beyond_next_index_are_rejected() {
    let temp = TestDir::new();
    for (index, name) in [
        "unexpected.txt",
        "parts/unexpected.parquet",
        "commits/part-1.json",
        "parts/part-00000000000000000002.parquet",
    ]
    .iter()
    .enumerate()
    {
        let (layout, input, output) = create_fixture(&temp, &format!("unknown-{index}"), 2);
        make_last_part_orphan(&output, 1);
        fs::write(output.join(name), b"unexpected data").unwrap();
        reject_without_changes(&layout, &input, &output, 2);
    }
    let (layout, input, output) = create_fixture(&temp, "unknown-directory", 2);
    fs::create_dir(output.join("unrecognized")).unwrap();
    reject_without_changes(&layout, &input, &output, 2);
}

#[test]
fn bootstrap_accepts_only_empty_initialization_structure() {
    let temp = TestDir::new();
    let input = temp.input(BINARY);
    let layout = parse_and_compile_copybook(COPYBOOK).unwrap();
    for level in 0..=4 {
        let output = temp.path(&format!("bootstrap-{level}"));
        fs::create_dir(&output).unwrap();
        if level >= 1 {
            fs::write(output.join(".m4.lock"), []).unwrap();
        }
        if level >= 2 {
            fs::create_dir(output.join("parts")).unwrap();
        }
        if level >= 3 {
            fs::create_dir(output.join("commits")).unwrap();
        }
        if level >= 4 {
            fs::write(output.join(".manifest.json.tmp"), b"{partial").unwrap();
        }
        convert_parts(&layout, &input, &output, 2, RecoveryMode::Resume).unwrap();
        assert_parts(&output, 2, &expected_batch());
        assert!(!output.join(".manifest.json.tmp").exists());
    }
    for (index, unexpected) in [
        "unknown.txt",
        "complete.json",
        "parts/part-00000000000000000000.parquet",
        "commits/part-00000000000000000000.json",
    ]
    .iter()
    .enumerate()
    {
        let output = temp.path(&format!("invalid-bootstrap-{index}"));
        fs::create_dir(&output).unwrap();
        fs::write(output.join(".m4.lock"), []).unwrap();
        fs::create_dir(output.join("parts")).unwrap();
        fs::create_dir(output.join("commits")).unwrap();
        fs::write(output.join(unexpected), b"unexpected content").unwrap();
        reject_without_changes(&layout, &input, &output, 2);
    }
}

#[test]
fn invalid_configuration_layout_and_partial_input_fail_before_output_creation() {
    let temp = TestDir::new();
    let input = temp.input(BINARY);
    let output = temp.path("absent");
    let mut layout = parse_and_compile_copybook(COPYBOOK).unwrap();
    assert!(matches!(
        convert_parts(&layout, &input, &output, 0, RecoveryMode::Create),
        Err(RecoveryError::Conversion(ConversionError::InvalidBatchSize))
    ));
    assert!(matches!(
        convert_parts(&layout, &input, &output, usize::MAX, RecoveryMode::Create),
        Err(RecoveryError::Conversion(ConversionError::CapacityExceeded))
    ));
    layout.record_length = 0;
    assert!(matches!(
        convert_parts(&layout, &input, &output, 2, RecoveryMode::Create),
        Err(RecoveryError::Conversion(ConversionError::Decode { .. }))
    ));
    let filler = parse_and_compile_copybook("       01 ROOT.\n       05 FILLER PIC X.\n").unwrap();
    assert!(matches!(
        convert_parts(&filler, &input, &output, 2, RecoveryMode::Create),
        Err(RecoveryError::Conversion(ConversionError::EmptySchema))
    ));
    let layout = parse_and_compile_copybook(COPYBOOK).unwrap();
    for remainder in 1..35 {
        fs::write(&input, &BINARY[..70 + remainder]).unwrap();
        let error = convert_parts(&layout, &input, &output, 2, RecoveryMode::Create).unwrap_err();
        assert!(
            matches!(error, RecoveryError::Conversion(ConversionError::TruncatedRecord { byte_offset: 70, actual, record_length: 35 }) if actual == remainder)
        );
        assert!(!output.exists());
    }
}

#[test]
fn resumed_decode_error_keeps_global_record_and_absolute_file_context() {
    let temp = TestDir::new();
    let mut bytes = BINARY.to_vec();
    bytes[83] = 0x40; // Corrupt BEFORE fingerprinting and committing the first batch.
    let input = temp.input(&bytes);
    let output = temp.path("decode-error");
    let layout = parse_and_compile_copybook(COPYBOOK).unwrap();
    assert!(convert_parts(&layout, &input, &output, 2, RecoveryMode::Create).is_err());
    assert!(receipt(&output, 0).exists());
    assert!(!receipt(&output, 1).exists());
    let confirmed = fs::read(part(&output, 0)).unwrap();
    let error = convert_parts(&layout, &input, &output, 2, RecoveryMode::Resume).unwrap_err();
    assert!(error.to_string().contains("input file byte 83"), "{error}");
    assert!(
        error
            .source()
            .unwrap()
            .downcast_ref::<ConversionError>()
            .is_some()
    );
    match error {
        RecoveryError::Conversion(ConversionError::Decode {
            batch_offset,
            source,
        }) => {
            assert_eq!(batch_offset, 70);
            assert!(matches!(
                source.kind,
                DecodeErrorKind::InvalidDisplayDigit {
                    offset: 1,
                    byte: 0x40
                }
            ));
            let context = source.context.unwrap();
            assert_eq!(context.record_index, 2);
            assert_eq!(context.byte_offset, 13);
            assert_eq!(context.span, m2c_pipeline::SourceSpan::new(6, 8));
            assert_eq!(
                context.field_path,
                "SAMPLE-RECORD.HEADER-GROUP.ACCOUNT-NUMBER"
            );
        }
        other => panic!("unexpected error: {other}"),
    }
    assert_eq!(fs::read(part(&output, 0)).unwrap(), confirmed);
    assert_cli_failure(
        temp.command(&input, &output, "2")
            .arg("--resume")
            .output()
            .unwrap(),
        "input file byte 83: record 2, field SAMPLE-RECORD.HEADER-GROUP.ACCOUNT-NUMBER, batch byte 13 (line 6, column 8): invalid DISPLAY digit 0x40 at field byte 1",
    );
}

#[test]
fn cli_rejects_missing_unknown_duplicate_and_invalid_arguments() {
    let temp = TestDir::new();
    let input = temp.input(BINARY);
    let output = temp.path("absent");
    for rows in ["0", "-1", "abc", "999999999999999999999999999999"] {
        assert_cli_failure(
            temp.command(&input, &output, rows).output().unwrap(),
            "batch-records",
        );
    }
    for args in [
        vec!["convert-parts"],
        vec!["convert-parts", "--input"],
        vec!["convert-parts", "--unknown", "value"],
        vec!["convert-parts", "--input", "a", "--input", "b"],
        vec!["convert-parts", "--resume", "--resume"],
    ] {
        let result = Command::new(env!("CARGO_BIN_EXE_m2c-pipeline"))
            .args(args)
            .output()
            .unwrap();
        assert_cli_failure(result, "");
    }
    let mut command = temp.command(&input, &output, "2");
    fs::write(
        temp.path("layout.cpy"),
        "       01 ROOT.\n       05 ITEMS OCCURS 2 TIMES.\n",
    )
    .unwrap();
    assert_cli_failure(command.output().unwrap(), "OCCURS");
    assert!(!output.exists());
}

#[test]
fn nonexistent_or_nonregular_inputs_and_missing_parent_return_errors() {
    let temp = TestDir::new();
    let layout = parse_and_compile_copybook(COPYBOOK).unwrap();
    let input = temp.input(BINARY);
    for (source, output) in [
        (temp.path("missing.bin"), temp.path("out-missing")),
        (temp.path.clone(), temp.path("out-directory")),
        (input.clone(), temp.path("missing-parent/out")),
    ] {
        let error = convert_parts(&layout, &source, &output, 2, RecoveryMode::Create).unwrap_err();
        assert!(
            matches!(
                error,
                RecoveryError::Io { .. }
                    | RecoveryError::InvalidState { .. }
                    | RecoveryError::Conversion(ConversionError::Io { .. })
            ),
            "{error}"
        );
        assert!(!output.exists());
    }
    let missing = temp.path("never-created");
    assert!(convert_parts(&layout, &input, &missing, 2, RecoveryMode::Resume).is_err());
    assert!(!missing.exists());
    assert_eq!(fs::read(input).unwrap(), BINARY);
}

#[test]
fn input_inside_output_directory_is_rejected_without_cleanup() {
    let temp = TestDir::new();
    let (layout, _input, output) = create_fixture(&temp, "contained-input", 2);
    let inside = output.join("input.bin");
    fs::write(&inside, BINARY).unwrap();
    reject_without_changes(&layout, &inside, &output, 2);
}

#[test]
fn links_in_managed_namespace_are_rejected_without_touching_target() {
    let temp = TestDir::new();
    let (layout, input, output) = create_fixture(&temp, "linked", 2);
    let external = temp.path("external");
    fs::create_dir(&external).unwrap();
    fs::write(external.join("keep.txt"), b"external data").unwrap();
    let link = output.join("parts");
    fs::rename(&link, temp.path("original-parts")).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&external, &link).unwrap();
    #[cfg(windows)]
    {
        // A local NTFS junction requires no symlink privilege. Paths are passed
        // as separate arguments and only creation is delegated to cmd's builtin.
        let result = Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&link)
            .arg(&external)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    assert!(is_link(&fs::symlink_metadata(&link).unwrap()));
    reject_without_changes(&layout, &input, &output, 2);
    assert_eq!(
        fs::read(external.join("keep.txt")).unwrap(),
        b"external data"
    );
    #[cfg(unix)]
    fs::remove_file(link).unwrap();
    #[cfg(windows)]
    fs::remove_dir(link).unwrap();
}

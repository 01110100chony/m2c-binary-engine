use std::error::Error;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use arrow_array::{Array, ArrayRef, Decimal128Array, Int64Array, RecordBatch, StringArray};
use arrow_schema::{DataType, Field, Schema};
use m2c_pipeline::{ConversionError, DecodeErrorKind, convert_file, parse_and_compile_copybook};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::basic::Compression;

const COPYBOOK: &str = include_str!("fixtures/sample_fixed.cpy");
const BINARY: &[u8] = include_bytes!("fixtures/sample_fixed.bin");

struct TestDir(PathBuf);
impl TestDir {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/m3-tests");
        fs::create_dir_all(&root).unwrap();
        loop {
            let path = root.join(format!(
                "{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("test directory: {error}"),
            }
        }
    }
    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
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
            .arg("convert")
            .arg("--copybook")
            .arg(copybook)
            .arg("--input")
            .arg(input)
            .arg("--output")
            .arg(output)
            .arg("--batch-records")
            .arg(rows);
        command
    }
}
impl Drop for TestDir {
    fn drop(&mut self) {
        // Only remove the unique directory successfully created by this instance.
        fs::remove_dir_all(&self.0).unwrap();
    }
}

// Independent constants from the annotated fixture, not the M2 decoder output.
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
    .map(|(name, data_type)| {
        Field::new(
            format!("SAMPLE-RECORD.HEADER-GROUP.{name}"),
            data_type,
            false,
        )
    })
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

fn assert_failure(output: Output, text: &str) {
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains(text), "{stderr}");
    assert!(!stderr.contains("panicked"), "{stderr}");
}

#[test]
fn cli_converts_known_fixture_across_batch_boundary_to_exact_parquet_values() {
    let temp = TestDir::new();
    let input = temp.input(BINARY);
    let output = temp.path("saída.parquet");
    let result = temp.command(&input, &output, "2").output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let reader = ParquetRecordBatchReaderBuilder::try_new(File::open(&output).unwrap()).unwrap();
    let expected = expected_batch();
    assert_eq!(reader.schema(), &expected.schema());
    assert_eq!(reader.metadata().file_metadata().num_rows(), 3);
    assert_eq!(reader.metadata().num_row_groups(), 2);
    for (group, rows) in reader.metadata().row_groups().iter().zip([2, 1]) {
        assert_eq!(group.num_rows(), rows);
        for column in group.columns() {
            assert_eq!(column.compression(), Compression::UNCOMPRESSED);
        }
    }
    let mut offset = 0;
    for batch in reader.with_batch_size(2).build().unwrap() {
        let batch = batch.unwrap();
        assert_eq!(batch, expected.slice(offset, batch.num_rows()));
        for column in batch.columns() {
            column.to_data().validate_full().unwrap();
        }
        offset += batch.num_rows();
    }
    assert_eq!(offset, 3);
}

#[test]
fn empty_input_produces_readable_parquet_with_exact_schema() {
    let temp = TestDir::new();
    let input = temp.input(&[]);
    let output = temp.path("empty.parquet");
    let result = temp.command(&input, &output, "2").output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let reader = ParquetRecordBatchReaderBuilder::try_new(File::open(output).unwrap()).unwrap();
    assert_eq!(reader.schema(), &expected_batch().schema());
    assert_eq!(reader.metadata().file_metadata().num_rows(), 0);
    assert_eq!(reader.build().unwrap().count(), 0);
}

#[test]
fn every_trailing_partial_record_is_rejected_with_absolute_position() {
    let temp = TestDir::new();
    let layout = parse_and_compile_copybook(COPYBOOK).unwrap();
    for remainder in 1..35 {
        let input = temp.input(&BINARY[..70 + remainder]);
        let output = temp.path(&format!("partial-{remainder}.parquet"));
        let error = convert_file(&layout, &input, &output, 2).unwrap_err();
        assert!(
            matches!(error, ConversionError::TruncatedRecord { byte_offset: 70, actual, record_length: 35 } if actual == remainder)
        );
    }
    let input = temp.input(&BINARY[..104]);
    assert_failure(
        temp.command(&input, &temp.path("cli-partial.parquet"), "2")
            .output()
            .unwrap(),
        "input file byte 70",
    );
}

#[test]
fn second_batch_decode_error_keeps_field_context_and_absolute_file_position() {
    let temp = TestDir::new();
    let mut bytes = BINARY.to_vec();
    bytes[83] = 0x40;
    let input = temp.input(&bytes);
    let layout = parse_and_compile_copybook(COPYBOOK).unwrap();
    let error = convert_file(&layout, &input, &temp.path("bad.parquet"), 2).unwrap_err();
    assert!(
        error
            .source()
            .unwrap()
            .downcast_ref::<m2c_pipeline::DecodeError>()
            .is_some()
    );
    assert!(error.to_string().contains("input file byte 83"));
    match error {
        ConversionError::Decode {
            batch_offset,
            source,
        } => {
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
        error => panic!("unexpected error: {error}"),
    }
    assert_failure(
        temp.command(&input, &temp.path("cli-bad.parquet"), "2")
            .output()
            .unwrap(),
        "input file byte 83",
    );
}

#[test]
fn cli_second_batch_diagnostic_reports_global_record_index() {
    let temp = TestDir::new();
    let mut bytes = BINARY.to_vec();
    bytes[83] = 0x40; // Record 2, first record in the second batch (2 + 1).
    let input = temp.input(&bytes);
    let result = temp
        .command(&input, &temp.path("global-record.parquet"), "2")
        .output()
        .unwrap();
    assert_failure(
        result,
        "input file byte 83: record 2, field SAMPLE-RECORD.HEADER-GROUP.ACCOUNT-NUMBER, batch byte 13 (line 6, column 8): invalid DISPLAY digit 0x40 at field byte 1",
    );

    let layout = parse_and_compile_copybook(COPYBOOK).unwrap();
    let error = m2c_pipeline::RecordDecoder::try_new(&layout)
        .unwrap()
        .decode_batch(&bytes[70..])
        .unwrap_err();
    assert_eq!(error.context.unwrap().record_index, 0);
}

#[test]
fn invalid_configuration_and_all_filler_layout_fail_before_output_creation() {
    let temp = TestDir::new();
    let input = temp.input(BINARY);
    let output = temp.path("absent.parquet");
    let mut layout = parse_and_compile_copybook(COPYBOOK).unwrap();
    assert!(matches!(
        convert_file(&layout, &input, &output, 0),
        Err(ConversionError::InvalidBatchSize)
    ));
    assert!(matches!(
        convert_file(&layout, &input, &output, usize::MAX),
        Err(ConversionError::CapacityExceeded)
    ));
    layout.record_length = 0;
    assert!(matches!(
        convert_file(&layout, &input, &output, 2),
        Err(ConversionError::Decode { .. })
    ));
    let filler = parse_and_compile_copybook("       01 ROOT.\n       05 FILLER PIC X.\n").unwrap();
    assert!(matches!(
        convert_file(&filler, &input, &output, 2),
        Err(ConversionError::EmptySchema)
    ));
    assert!(!output.exists());
}

#[test]
fn io_errors_preserve_their_cause_and_existing_files_are_never_overwritten() {
    let temp = TestDir::new();
    let input = temp.input(BINARY);
    let layout = parse_and_compile_copybook(COPYBOOK).unwrap();
    for (source, output, kind) in [
        (
            temp.path("missing.bin"),
            temp.path("absent.parquet"),
            std::io::ErrorKind::NotFound,
        ),
        (
            input.clone(),
            temp.path("missing-directory/out.parquet"),
            std::io::ErrorKind::NotFound,
        ),
        (
            input.clone(),
            input.clone(),
            std::io::ErrorKind::AlreadyExists,
        ),
    ] {
        let error = convert_file(&layout, &source, &output, 2).unwrap_err();
        assert_eq!(
            error
                .source()
                .unwrap()
                .downcast_ref::<std::io::Error>()
                .unwrap()
                .kind(),
            kind
        );
    }
    let output = temp.path("existing.parquet");
    fs::write(&output, b"keep this file").unwrap();
    assert_failure(
        temp.command(&input, &output, "2").output().unwrap(),
        "create output",
    );
    assert_eq!(fs::read(output).unwrap(), b"keep this file");
    assert_eq!(fs::read(input).unwrap(), BINARY);
}

#[test]
fn cli_rejects_missing_unknown_duplicate_and_invalid_arguments() {
    let temp = TestDir::new();
    let input = temp.input(BINARY);
    let output = temp.path("absent.parquet");
    for value in ["0", "-1", "abc", "999999999999999999999999999999"] {
        assert_failure(
            temp.command(&input, &output, value).output().unwrap(),
            "batch-records",
        );
    }
    for (args, message) in [
        (vec![], "usage:"),
        (vec!["convert"], "usage:"),
        (vec!["convert", "--input"], "missing value"),
        (
            vec!["convert", "--unknown", "value"],
            "unknown or duplicate",
        ),
        (
            vec!["convert", "--input", "a", "--input", "b"],
            "unknown or duplicate",
        ),
    ] {
        assert_failure(
            Command::new(env!("CARGO_BIN_EXE_m2c-pipeline"))
                .args(args)
                .output()
                .unwrap(),
            message,
        );
    }
    assert!(!output.exists());
}

#[test]
fn cli_rejects_invalid_copybook_without_creating_output() {
    let temp = TestDir::new();
    let input = temp.input(BINARY);
    let output = temp.path("absent.parquet");
    let mut command = temp.command(&input, &output, "2");
    fs::write(
        temp.path("layout.cpy"),
        "       01 ROOT.\n       05 ITEMS OCCURS 2 TIMES.\n",
    )
    .unwrap();
    assert_failure(command.output().unwrap(), "OCCURS");
    assert!(!output.exists());
}

#[test]
fn decimal_precision_18_scale_zero_and_17_and_schema_metadata_roundtrip() {
    let temp = TestDir::new();
    let mut layout = parse_and_compile_copybook(concat!(
        "       01 ROOT.\n",
        "       05 WHOLE PIC 9(18) COMP-3.\n",
        "       05 FRACTION PIC S9(1)V9(17) COMP.\n",
    ))
    .unwrap();
    layout
        .arrow_schema
        .metadata
        .insert("origin".into(), "M3 test".into());
    const MAX: i64 = 999_999_999_999_999_999;
    let mut bytes = vec![0x09, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x9F];
    bytes.extend((-MAX).to_be_bytes());
    let input = temp.input(&bytes);
    let output = temp.path("precision.parquet");
    convert_file(&layout, &input, &output, 1).unwrap();
    let reader = ParquetRecordBatchReaderBuilder::try_new(File::open(output).unwrap()).unwrap();
    assert_eq!(reader.schema().as_ref(), &layout.arrow_schema);
    let batch = reader.build().unwrap().next().unwrap().unwrap();
    assert_eq!(batch.num_rows(), 1);
    for (column, scale, value) in [(0, 0, i128::from(MAX)), (1, 17, -i128::from(MAX))] {
        let array = batch
            .column(column)
            .as_any()
            .downcast_ref::<Decimal128Array>()
            .unwrap();
        assert_eq!(array.data_type(), &DataType::Decimal128(18, scale));
        assert_eq!(array.value(0), value);
    }
}

#[test]
fn failure_after_successful_batch_leaves_unfinalized_file() {
    let layout_src = "       01 ROOT.\n       05 VAL PIC 9(2).\n";
    let layout = parse_and_compile_copybook(layout_src).unwrap();

    let mut input_data = Vec::new();
    input_data.extend_from_slice(&[0xF1, 0xF2]); // rec 0, valid EBCDIC "12"
    input_data.extend_from_slice(&[0xF3, 0xF4]); // rec 1, valid EBCDIC "34"
    input_data.extend_from_slice(b"XX"); // rec 2, invalid DISPLAY
    input_data.extend_from_slice(&[0xF7, 0xF8]); // rec 3, valid

    let test_dir = std::env::temp_dir().join("audit_m3");
    std::fs::create_dir_all(&test_dir).unwrap();

    let input_path = test_dir.join("input.bin");
    std::fs::write(&input_path, &input_data).unwrap();
    let output_path = test_dir.join("output.parquet");
    let _ = std::fs::remove_file(&output_path);

    // Run conversion with batch size 2
    let error = convert_file(&layout, &input_path, &output_path, 2).unwrap_err();

    // Verify conversion returns failure
    assert!(matches!(error, ConversionError::Decode { .. }));

    // Verify partial output cannot masquerade as successfully finalized Parquet
    let reader_result = ParquetRecordBatchReaderBuilder::try_new(File::open(&output_path).unwrap());
    assert!(
        reader_result.is_err(),
        "Partial file should not be readable as valid Parquet"
    );

    let err_str = match reader_result {
        Err(e) => e.to_string(),
        _ => unreachable!(),
    };

    assert!(
        err_str.contains("footer") || err_str.contains("too small"),
        "Unexpected error: {}",
        err_str
    );

    // Verify file resources are released (file is accessible and can be deleted)
    std::fs::remove_file(&output_path).expect("File should be unlocked and deletable");
    std::fs::remove_file(&input_path).unwrap();
}

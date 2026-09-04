use arrow_array::{Array, Decimal128Array, Int64Array, RecordBatch, StringArray};
use arrow_schema::{DataType, Field, Schema};
use m2c_pipeline::{convert_file, parse_and_compile_copybook};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use std::fs::{self, File};
use std::path::Path;
use std::sync::Arc;

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
    let columns: Vec<Arc<dyn Array>> = vec![
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

#[test]
fn audit_batch_sizes() {
    let copybook = include_str!("fixtures/sample_fixed.cpy");
    let binary = include_bytes!("fixtures/sample_fixed.bin"); // 3 records (105 bytes)
    let layout = parse_and_compile_copybook(copybook).unwrap();

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/audit");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();

    let input = root.join("input.bin");
    fs::write(&input, binary).unwrap();
    let expected = expected_batch();

    // Batch sizes 1, 2, 3, exact divisors, non-divisors, and batch sizes larger than total record count
    for batch_size in [1, 2, 3, 4, 5, 10, 100] {
        let output = root.join(format!("output_{batch_size}.parquet"));
        convert_file(&layout, &input, &output, batch_size).unwrap();

        let reader =
            ParquetRecordBatchReaderBuilder::try_new(File::open(&output).unwrap()).unwrap();

        let mut total_rows = 0;
        let mut offset = 0;
        for batch in reader.with_batch_size(3).build().unwrap() {
            let batch = batch.unwrap();
            total_rows += batch.num_rows();

            // verify records are not skipped, duplicated, reordered, or processed twice
            assert_eq!(
                batch,
                expected.slice(offset, batch.num_rows()),
                "Mismatch for batch_size {}",
                batch_size
            );
            offset += batch.num_rows();
        }
        assert_eq!(
            total_rows, 3,
            "Total rows mismatch for batch size {}",
            batch_size
        );
    }
}

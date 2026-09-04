use std::fs;
use m2c_pipeline::{parse_and_compile_copybook, convert_file};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use arrow_array::Array;

#[test]
fn reusable_buffer_does_not_contaminate_final_partial_batch() {
    let layout = parse_and_compile_copybook(
        "       01 ROOT.\n       05 VAL PIC X(4).\n"
    ).unwrap();

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/m3-test-reusable-buf");
    let _ = fs::create_dir_all(&root);
    let input = root.join("input.bin");
    let output = root.join("output.parquet");

    let _ = fs::remove_file(&output);

    let mut data = Vec::new();
    data.extend_from_slice(b"\xC1\xC1\xC1\xC1"); // AAAA
    data.extend_from_slice(b"\xC2\xC2\xC2\xC2"); // BBBB
    data.extend_from_slice(b"\xC3\xC3\xC3\xC3"); // CCCC

    fs::write(&input, data).unwrap();

    convert_file(&layout, &input, &output, 2).unwrap();

    let file = std::fs::File::open(&output).unwrap();
    let reader = ParquetRecordBatchReaderBuilder::try_new(file).unwrap();

    let metadata = reader.metadata();
    assert_eq!(metadata.num_row_groups(), 2);

    let mut batches = Vec::new();
    // Use the same batch size to see distinct batches
    for batch in reader.with_batch_size(2).build().unwrap() {
        batches.push(batch.unwrap());
    }

    assert_eq!(batches.len(), 2);

    // First batch: AAAA, BBBB
    let first_batch = &batches[0];
    assert_eq!(first_batch.num_rows(), 2);
    let col1 = first_batch.column(0).as_any().downcast_ref::<arrow_array::StringArray>().unwrap();
    assert_eq!(col1.value(0), "AAAA");
    assert_eq!(col1.value(1), "BBBB");

    // Second batch: CCCC
    // Should NOT contain any stale data from BBBB
    let second_batch = &batches[1];
    assert_eq!(second_batch.num_rows(), 1);
    let col2 = second_batch.column(0).as_any().downcast_ref::<arrow_array::StringArray>().unwrap();
    assert_eq!(col2.value(0), "CCCC");

    let _ = fs::remove_file(&input);
    let _ = fs::remove_file(&output);
}

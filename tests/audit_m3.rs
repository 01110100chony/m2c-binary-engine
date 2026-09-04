use m2c_pipeline::{ConversionError, convert_file, parse_and_compile_copybook};
use std::path::PathBuf;

#[test]
fn test_m3_arithmetic_and_allocation_boundaries() {
    let layout =
        parse_and_compile_copybook("       01 ROOT.\n       05 COUNT-FIELD PIC 9(2).\n").unwrap();
    let input = PathBuf::from("tests/fixtures/sample_fixed.bin");
    let output = PathBuf::from("target/m3-tests/audit_output.parquet");
    let _ = std::fs::remove_file(&output);

    // batch_records = 0
    let res = convert_file(&layout, &input, &output, 0);
    assert!(matches!(res, Err(ConversionError::InvalidBatchSize)));

    // batch_records = 1 (should attempt I/O)
    // we don't care if it errors due to IO, we just want to know it didn't panic
    let _ = convert_file(&layout, &input, &output, 1);

    // extremely large batch_records
    let res = convert_file(&layout, &input, &output, usize::MAX);
    assert!(matches!(res, Err(ConversionError::CapacityExceeded)));

    // record_length * batch_records overflow
    let max_batch = usize::MAX / layout.record_length + 1;
    let res = convert_file(&layout, &input, &output, max_batch);
    assert!(matches!(res, Err(ConversionError::CapacityExceeded)));

    // Test conversion bounds overflow
    let res = convert_file(&layout, &input, &output, (isize::MAX as usize) + 1);
    assert!(matches!(res, Err(ConversionError::CapacityExceeded)));
}

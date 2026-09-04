use m2c_pipeline::{ConversionError, convert_file, parse_and_compile_copybook};
use std::fs::{self, File};
use std::path::Path;

#[cfg(unix)]
use std::os::unix::fs::symlink;

const COPYBOOK: &str = "       01 ROOT.\n       05 VAL PIC X.\n";

#[test]
fn audit_m3_output_file_safety() {
    let temp = tempfile::tempdir().unwrap();
    let temp_path = temp.path();

    let layout = parse_and_compile_copybook(COPYBOOK).unwrap();
    let input = temp_path.join("input.bin");
    fs::write(&input, b"1").unwrap();

    // 1. Existing zero-byte output
    let zero_byte = temp_path.join("zero.parquet");
    File::create(&zero_byte).unwrap();
    let err = convert_file(&layout, &input, &zero_byte, 1).unwrap_err();
    assert!(matches!(err, ConversionError::Io { .. }));

    // 2. Existing valid Parquet output
    let valid_parquet = temp_path.join("valid.parquet");
    convert_file(&layout, &input, &valid_parquet, 1).unwrap();
    let err = convert_file(&layout, &input, &valid_parquet, 1).unwrap_err();
    assert!(matches!(err, ConversionError::Io { .. }));

    // 3. Input path exactly equal to output path
    let err = convert_file(&layout, &input, &input, 1).unwrap_err();
    assert!(matches!(err, ConversionError::Io { .. }));

    // 4. Equivalent relative/absolute paths
    let current_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp_path).unwrap();
    let err = convert_file(&layout, Path::new("input.bin"), Path::new("input.bin"), 1).unwrap_err();
    assert!(matches!(err, ConversionError::Io { .. }));
    let err = convert_file(&layout, Path::new("input.bin"), &input, 1).unwrap_err();
    assert!(matches!(err, ConversionError::Io { .. }));
    std::env::set_current_dir(current_dir).unwrap();

    // 5. Symlink aliases
    #[cfg(unix)]
    {
        let symlink_in = temp_path.join("symlink_in.bin");
        symlink(&input, &symlink_in).unwrap();
        let err = convert_file(&layout, &input, &symlink_in, 1).unwrap_err();
        assert!(matches!(err, ConversionError::Io { .. }));

        let broken_symlink = temp_path.join("broken.parquet");
        symlink(temp_path.join("non_existent"), &broken_symlink).unwrap();
        let err = convert_file(&layout, &input, &broken_symlink, 1).unwrap_err();
        assert!(matches!(err, ConversionError::Io { .. }));
    }

    // 6. Hard-link aliases
    let hardlink_in = temp_path.join("hardlink_in.bin");
    fs::hard_link(&input, &hardlink_in).unwrap();
    let err = convert_file(&layout, &input, &hardlink_in, 1).unwrap_err();
    assert!(matches!(err, ConversionError::Io { .. }));
}

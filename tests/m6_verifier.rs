#[path = "../examples/support/verify.rs"]
mod verify;
use std::{fs, path::PathBuf};
#[test]
fn external_verifier_checks_values_even_with_consistent_receipt_hashes() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join(format!("m6-verifier-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let input = root.join("input");
    fs::write(&input, include_bytes!("fixtures/sample_fixed.bin")).unwrap();
    let layout =
        m2c_pipeline::parse_and_compile_copybook(include_str!("fixtures/sample_fixed.cpy"))
            .unwrap();
    let job = root.join("job");
    m2c_pipeline::convert_parts(&layout, &input, &job, 2, m2c_pipeline::RecoveryMode::Create)
        .unwrap();
    verify::m4(&job, &input, 3, 2).unwrap();
    verify::roundtrip(&input, &input).unwrap();
    let part = job.join("parts/part-00000000000000000000.parquet");
    let receipt_path = job.join("commits/part-00000000000000000000.json");
    let mut altered = include_bytes!("fixtures/sample_fixed.bin").to_vec();
    altered[0] = 0xC2;
    let bad_input = root.join("bad");
    fs::write(&bad_input, &altered[..70]).unwrap();
    let replacement = root.join("replacement.parquet");
    m2c_pipeline::convert_file(&layout, &bad_input, &replacement, 2).unwrap();
    fs::copy(&replacement, &part).unwrap();
    let (size, hash) = verify::digest(&part).unwrap();
    let mut receipt: serde_json::Value =
        serde_json::from_slice(&fs::read(&receipt_path).unwrap()).unwrap();
    receipt["parquet_bytes"] = size.into();
    receipt["parquet_sha256"] = hash.into();
    fs::write(&receipt_path, serde_json::to_vec(&receipt).unwrap()).unwrap();
    assert!(
        verify::m4(&job, &input, 3, 2)
            .unwrap_err()
            .to_string()
            .contains("value")
    );
}

#[test]
fn external_verifier_rejects_malformed_metadata_and_unknown_artifacts() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join(format!("m6-verifier-metadata-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let input = root.join("input");
    fs::write(&input, include_bytes!("fixtures/sample_fixed.bin")).unwrap();
    let layout =
        m2c_pipeline::parse_and_compile_copybook(include_str!("fixtures/sample_fixed.cpy"))
            .unwrap();
    let job = root.join("job");
    m2c_pipeline::convert_parts(&layout, &input, &job, 2, m2c_pipeline::RecoveryMode::Create)
        .unwrap();
    verify::m4(&job, &input, 3, 2).unwrap();
    let manifest = job.join("manifest.json");
    let original = fs::read(&manifest).unwrap();
    let mut duplicated = b"{\"version\":1,".to_vec();
    duplicated.extend_from_slice(&original[1..]);
    let mut trailing = original.clone();
    trailing.extend_from_slice(b" trailing");
    for bad in [duplicated, trailing, vec![b' '; 4097], b"{}".to_vec()] {
        fs::write(&manifest, bad).unwrap();
        assert!(verify::m4(&job, &input, 3, 2).is_err());
    }
    fs::write(&manifest, original).unwrap();
    fs::write(job.join("unknown"), []).unwrap();
    assert!(verify::m4(&job, &input, 3, 2).is_err());
}

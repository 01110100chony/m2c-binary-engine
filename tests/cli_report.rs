use serde_json::Value;
use std::{fs, path::PathBuf, process::Command};

fn root() -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/m6-report-tests");
    fs::create_dir_all(&root).unwrap();
    let path = root.join(format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&path).unwrap();
    path
}
fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_m2c-pipeline"))
}
#[test]
fn reports_conversion_resume_empty_and_failed_operations_without_paths() {
    let root = root();
    let input = root.join("private input.bin");
    fs::write(&input, include_bytes!("fixtures/sample_fixed.bin")).unwrap();
    for (command, flag, output) in [
        ("convert", "--output", root.join("result.parquet")),
        ("convert-parts", "--output-dir", root.join("parts")),
    ] {
        let run = |resume: bool, json: bool| {
            let mut c = cli();
            c.arg(command)
                .args(["--copybook", "tests/fixtures/sample_fixed.cpy", "--input"])
                .arg(&input)
                .arg(flag)
                .arg(&output)
                .args(["--batch-records", "2"]);
            if resume {
                c.arg("--resume");
            }
            if json {
                c.arg("--report-json");
            }
            c.output().unwrap()
        };
        let out = run(false, true);
        assert!(out.status.success(), "{:?}", out);
        let report: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(report["dataset_records"], 3);
        assert_eq!(report["input_bytes"], 105);
        assert!(!String::from_utf8_lossy(&out.stdout).contains("private"));
        if command == "convert-parts" {
            let out = run(true, true);
            assert!(out.status.success());
            let r: Value = serde_json::from_slice(&out.stdout).unwrap();
            assert_eq!(r["mode"], "resume");
            assert_eq!(r["dataset_parts"], 2);
            assert!(r["output_bytes"].is_null());
        }
        let failed = run(false, true);
        assert!(!failed.status.success());
        let r: Value = serde_json::from_slice(&failed.stdout).unwrap();
        assert!(r["publication"].is_null());
        assert!(r["output_bytes"].is_null());
        assert!(r["dataset_records"].is_null());
    }
    fs::write(&input, []).unwrap();
    let out = cli()
        .args([
            "convert",
            "--report-json",
            "--copybook",
            "tests/fixtures/sample_fixed.cpy",
            "--input",
        ])
        .arg(&input)
        .arg("--output")
        .arg(root.join("empty.parquet"))
        .args(["--batch-records", "1"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let r: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(r["dataset_records"], 0);
}
#[test]
fn report_flag_errors_and_disabled_mode() {
    for args in [
        vec!["convert", "--report-json"],
        vec!["convert", "--report-json", "--report-json"],
        vec!["convert", "--unexpected", "--report-json"],
    ] {
        let out = cli().args(args).output().unwrap();
        assert!(!out.status.success());
        let r: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(r["error_category"], "arguments");
    }
    let out = cli().arg("convert").output().unwrap();
    assert!(out.stdout.is_empty());
}

#[test]
fn report_flag_as_path_is_not_stripped_and_late_errors_do_not_claim_progress() {
    let root = root();
    let input = root.join("--report-json");
    fs::write(&input, include_bytes!("fixtures/sample_fixed.bin")).unwrap();
    let copybook =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sample_fixed.cpy");
    let run = |name: &str, report: bool| {
        let mut command = cli();
        command
            .current_dir(&root)
            .args(["convert", "--input", "--report-json", "--copybook"])
            .arg(&copybook)
            .arg("--output")
            .arg(root.join(name))
            .args(["--batch-records", "2"]);
        if report {
            command.arg("--report-json");
        }
        command.output().unwrap()
    };
    let plain = run("plain", false);
    assert!(plain.status.success());
    assert!(plain.stdout.is_empty());
    let out = run("json", true);
    assert!(out.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&out.stdout).unwrap()["dataset_records"],
        3
    );
    let mut invalid = include_bytes!("fixtures/sample_fixed.bin").to_vec();
    invalid[83] = 0x40;
    fs::write(&input, invalid).unwrap();
    let out = run("partial", true);
    assert!(!out.status.success());
    let value: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(value["error_category"], "conversion");
    assert!(value["dataset_records"].is_null() && value["output_bytes"].is_null());
    assert!(root.join("partial").exists());
}
#[cfg(all(feature = "pqc", windows))]
#[test]
fn reports_m5_outcomes_without_secret_or_path() {
    let root = root();
    let keys = root.join("keys");
    let out = cli()
        .args(["keygen", "--report-json", "--output-dir"])
        .arg(&keys)
        .output()
        .unwrap();
    assert!(out.status.success(), "{:?}", out);
    let r: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(r["publication"]["public_key"], "published");
    let input = root.join("input");
    fs::write(&input, b"secret message").unwrap();
    let envelope = root.join("envelope");
    for (command, source, keyflag, key, dest) in [
        (
            "protect",
            input,
            "--public-key",
            keys.join("public.key"),
            envelope.clone(),
        ),
        (
            "unprotect",
            envelope,
            "--secret-key",
            keys.join("secret.key"),
            root.join("recovered"),
        ),
    ] {
        let out = cli()
            .arg(command)
            .args(["--report-json", "--input"])
            .arg(source)
            .arg(keyflag)
            .arg(key)
            .arg("--output")
            .arg(dest)
            .output()
            .unwrap();
        assert!(out.status.success(), "{:?}", out);
        let r: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(r["publication"]["output"], "published");
        assert!(!String::from_utf8_lossy(&out.stdout).contains("secret"));
    }
}

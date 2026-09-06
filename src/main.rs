#[cfg(feature = "pqc")]
use m2c_pipeline::protection::{
    KeyGenerationOutcome, ProtectionOutcome, ProtectionWarning, PublicationStatus,
    generate_keypair, protect_file, unprotect_file,
};
use m2c_pipeline::{RecoveryMode, convert_file, convert_parts, parse_and_compile_copybook};
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;
mod cli_report;
use cli_report::Report;

const USAGE: &str = "usage: m2c-pipeline convert --copybook <file> --input <file> --output <file> --batch-records <N> [--report-json]";
const PARTS_USAGE: &str = "usage: m2c-pipeline convert-parts --copybook <file> --input <file> --output-dir <dir> --batch-records <N> [--resume] [--report-json]";
#[cfg(feature = "pqc")]
const KEYGEN_USAGE: &str = "usage: m2c-pipeline keygen --output-dir <dir> [--report-json]";
#[cfg(feature = "pqc")]
const PROTECT_USAGE: &str = "usage: m2c-pipeline protect --input <file> --public-key <file> --output <file> [--report-json]";
#[cfg(feature = "pqc")]
const UNPROTECT_USAGE: &str = "usage: m2c-pipeline unprotect --input <file> --secret-key <file> --output <file> [--report-json]";
struct Args {
    copybook: PathBuf,
    input: PathBuf,
    output: PathBuf,
    batch_records: usize,
    recovery_mode: Option<RecoveryMode>,
}

#[cfg(feature = "pqc")]
struct ProtectionArgs {
    input: PathBuf,
    key: PathBuf,
    output: PathBuf,
}

#[cfg(feature = "pqc")]
fn parse_keygen_args(mut args: impl Iterator<Item = OsString>) -> Result<PathBuf, String> {
    let flag = args.next().ok_or(KEYGEN_USAGE)?;
    let output = args
        .next()
        .ok_or_else(|| format!("missing value for {flag:?}\n{KEYGEN_USAGE}"))?;
    if flag != "--output-dir" || args.next().is_some() {
        return Err(format!(
            "unknown, duplicate or extra argument {flag:?}\n{KEYGEN_USAGE}"
        ));
    }
    Ok(output.into())
}

#[cfg(feature = "pqc")]
fn parse_protection_args(
    mut args: impl Iterator<Item = OsString>,
    unprotect: bool,
) -> Result<ProtectionArgs, String> {
    let usage = if unprotect {
        UNPROTECT_USAGE
    } else {
        PROTECT_USAGE
    };
    let (mut input, mut key, mut output) = (None, None, None);
    while let Some(flag) = args.next() {
        let value = args
            .next()
            .ok_or_else(|| format!("missing value for {flag:?}\n{usage}"))?;
        match flag.to_str() {
            Some("--input") if input.is_none() => input = Some(value.into()),
            Some("--public-key") if !unprotect && key.is_none() => key = Some(value.into()),
            Some("--secret-key") if unprotect && key.is_none() => key = Some(value.into()),
            Some("--output") if output.is_none() => output = Some(value.into()),
            _ => return Err(format!("unknown or duplicate argument {flag:?}\n{usage}")),
        }
    }
    Ok(ProtectionArgs {
        input: input.ok_or(usage)?,
        key: key.ok_or(usage)?,
        output: output.ok_or(usage)?,
    })
}

#[cfg(feature = "pqc")]
fn report_warning(warning: &ProtectionWarning) {
    eprintln!("m2c-pipeline: warning: {warning}");
}

#[cfg(feature = "pqc")]
fn report_publication(label: &str, status: &PublicationStatus) {
    if let PublicationStatus::PublishedWithStagingResidue(path) = status {
        eprintln!(
            "m2c-pipeline: warning: {label} was published, but staging residue remains at {}",
            path.display()
        );
    }
}

#[cfg(feature = "pqc")]
fn report_protection_outcome(outcome: &ProtectionOutcome) {
    report_publication("output", &outcome.publication);
    for warning in &outcome.warnings {
        report_warning(warning);
    }
}

#[cfg(feature = "pqc")]
fn report_keygen_outcome(outcome: &KeyGenerationOutcome) {
    report_publication("public key", &outcome.public_key);
    report_publication("secret key", &outcome.secret_key);
    for warning in &outcome.warnings {
        report_warning(warning);
    }
}

#[cfg(feature = "pqc")]
fn run_protection_command(
    command: &str,
    args: impl Iterator<Item = OsString>,
    report: &mut Report,
) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        "keygen" => {
            let output = parse_keygen_args(args)?;
            report.error_category = Some("protection");
            let outcome = generate_keypair(&output)?;
            report.keygen(&outcome);
            report_keygen_outcome(&outcome);
        }
        "protect" | "unprotect" => {
            let unprotect = command == "unprotect";
            let args = parse_protection_args(args, unprotect)?;
            report.files(&args.input, Some(&args.output));
            report.error_category = Some("protection");
            let outcome = if unprotect {
                unprotect_file(&args.input, &args.key, &args.output)?
            } else {
                protect_file(&args.input, &args.key, &args.output)?
            };
            report.protection(&outcome);
            report_protection_outcome(&outcome);
        }
        _ => unreachable!("caller filters M5 command names"),
    }
    Ok(())
}

fn parse_args(mut args: impl Iterator<Item = OsString>) -> Result<Args, String> {
    let parts = match args.next().as_deref().and_then(std::ffi::OsStr::to_str) {
        Some("convert") => false,
        Some("convert-parts") => true,
        _ => return Err(format!("{USAGE}\n{PARTS_USAGE}")),
    };
    let usage = if parts { PARTS_USAGE } else { USAGE };
    let (mut copybook, mut input, mut output, mut batch_records) = (None, None, None, None);
    let mut resume = false;
    while let Some(flag) = args.next() {
        if parts && flag == "--resume" {
            if resume {
                return Err(format!("unknown or duplicate argument {flag:?}\n{usage}"));
            }
            resume = true;
            continue;
        }
        let value = args
            .next()
            .ok_or_else(|| format!("missing value for {flag:?}\n{usage}"))?;
        match flag.to_str() {
            Some("--copybook") if copybook.is_none() => copybook = Some(PathBuf::from(value)),
            Some("--input") if input.is_none() => input = Some(PathBuf::from(value)),
            Some("--output") if !parts && output.is_none() => output = Some(PathBuf::from(value)),
            Some("--output-dir") if parts && output.is_none() => {
                output = Some(PathBuf::from(value))
            }
            Some("--batch-records") if batch_records.is_none() => {
                batch_records = Some(
                    value
                        .to_str()
                        .and_then(|value| value.parse::<usize>().ok())
                        .filter(|&value| value > 0)
                        .ok_or_else(|| {
                            "batch-records must be a positive addressable integer".to_owned()
                        })?,
                );
            }
            _ => return Err(format!("unknown or duplicate argument {flag:?}\n{usage}")),
        }
    }
    Ok(Args {
        copybook: copybook.ok_or(usage)?,
        input: input.ok_or(usage)?,
        output: output.ok_or(usage)?,
        batch_records: batch_records.ok_or(usage)?,
        recovery_mode: parts.then_some(if resume {
            RecoveryMode::Resume
        } else {
            RecoveryMode::Create
        }),
    })
}

fn run(
    mut raw: impl Iterator<Item = OsString>,
    report: &mut Report,
) -> Result<(), Box<dyn std::error::Error>> {
    let command = raw.next();
    #[cfg(feature = "pqc")]
    if let Some(command_name) = command.as_deref().and_then(std::ffi::OsStr::to_str)
        && matches!(command_name, "keygen" | "protect" | "unprotect")
    {
        return run_protection_command(command_name, raw, report);
    }
    let args = parse_args(command.into_iter().chain(raw))?;
    report.batch_records = Some(args.batch_records);
    report.mode = args.recovery_mode.map(|m| match m {
        RecoveryMode::Create => "create",
        RecoveryMode::Resume => "resume",
    });
    report.files(
        &args.input,
        args.recovery_mode
            .is_none()
            .then_some(args.output.as_path()),
    );
    report.error_category = Some("input_io");
    let copybook = std::fs::read_to_string(&args.copybook)
        .map_err(|error| format!("read copybook {}: {error}", args.copybook.display()))?;
    report.error_category = Some("copybook");
    let layout = parse_and_compile_copybook(&copybook)?;
    report.record_length = Some(layout.record_length);
    report.error_category = Some(if args.recovery_mode.is_some() {
        "recovery"
    } else {
        "conversion"
    });
    match args.recovery_mode {
        Some(mode) => convert_parts(&layout, &args.input, &args.output, args.batch_records, mode)?,
        None => convert_file(&layout, &args.input, &args.output, args.batch_records)?,
    }
    Ok(())
}

fn main() -> ExitCode {
    let start = Instant::now();
    let mut raw = std::env::args_os().skip(1);
    let command = raw.next().unwrap_or_default();
    let name = command.to_str().unwrap_or("");
    let recognized = matches!(name, "convert" | "convert-parts")
        || (cfg!(feature = "pqc") && matches!(name, "keygen" | "protect" | "unprotect"));
    let (mut filtered, mut count, mut value) = (Vec::new(), 0, false);
    for arg in raw {
        if recognized && !value && arg == "--report-json" {
            count += 1;
            continue;
        }
        if value {
            value = false;
        } else {
            value = matches!(
                arg.to_str(),
                Some(
                    "--input"
                        | "--output"
                        | "--output-dir"
                        | "--copybook"
                        | "--batch-records"
                        | "--public-key"
                        | "--secret-key"
                )
            );
        }
        filtered.push(arg);
    }
    let mut report = Report::new(name, count > 0);
    let result = if count > 1 {
        Err("duplicate argument --report-json".into())
    } else {
        run(std::iter::once(command).chain(filtered), &mut report)
    };
    let elapsed = start.elapsed();
    report.finish(result.is_ok(), elapsed);
    if report.enabled && report.write(&mut std::io::stdout().lock()).is_err() {
        use std::io::Write;
        let _ = writeln!(
            std::io::stderr().lock(),
            "m2c-pipeline: warning: JSON report unavailable"
        );
    }
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("m2c-pipeline: {error}");
            ExitCode::FAILURE
        }
    }
}

use m2c_pipeline::{RecoveryMode, convert_file, convert_parts, parse_and_compile_copybook};
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "usage: m2c-pipeline convert --copybook <file> --input <file> --output <file> --batch-records <N>";
const PARTS_USAGE: &str = "usage: m2c-pipeline convert-parts --copybook <file> --input <file> --output-dir <dir> --batch-records <N> [--resume]";
struct Args {
    copybook: PathBuf,
    input: PathBuf,
    output: PathBuf,
    batch_records: usize,
    recovery_mode: Option<RecoveryMode>,
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

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args = parse_args(std::env::args_os().skip(1))?;
    let copybook = std::fs::read_to_string(&args.copybook)
        .map_err(|error| format!("read copybook {}: {error}", args.copybook.display()))?;
    let layout = parse_and_compile_copybook(&copybook)?;
    match args.recovery_mode {
        Some(mode) => convert_parts(&layout, &args.input, &args.output, args.batch_records, mode)?,
        None => convert_file(&layout, &args.input, &args.output, args.batch_records)?,
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("m2c-pipeline: {error}");
            ExitCode::FAILURE
        }
    }
}

#[path = "support/verify.rs"]
mod verify;
use std::{collections::BTreeMap, error::Error, path::PathBuf};
fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args_os().skip(1);
    let mut flags = BTreeMap::new();
    while let Some(key) = args.next() {
        let key = key.into_string().map_err(|_| "nonunicode flag")?;
        if ![
            "--kind",
            "--input",
            "--output",
            "--records",
            "--batch-records",
        ]
        .contains(&key.as_str())
        {
            return Err("unknown flag".into());
        }
        let value = args.next().ok_or("missing value")?;
        if flags.insert(key, value).is_some() {
            return Err("duplicate flag".into());
        }
    }
    let kind = flags
        .get("--kind")
        .and_then(|s| s.to_str())
        .ok_or("missing kind")?;
    let input = PathBuf::from(flags.get("--input").ok_or("missing input")?);
    let output = PathBuf::from(flags.get("--output").ok_or("missing output")?);
    if kind == "roundtrip" {
        verify::roundtrip(&input, &output)?;
    } else {
        let records: u64 = flags
            .get("--records")
            .and_then(|s| s.to_str())
            .ok_or("missing records")?
            .parse()?;
        let batch: u64 = flags
            .get("--batch-records")
            .and_then(|s| s.to_str())
            .ok_or("missing batch")?
            .parse()?;
        match kind {
            "m3" => verify::parquet(&output, 0, records, batch)?,
            "m4" => verify::m4(&output, &input, records, batch)?,
            _ => return Err("unknown kind".into()),
        }
    }
    println!("{{\"verified\":true}}");
    Ok(())
}

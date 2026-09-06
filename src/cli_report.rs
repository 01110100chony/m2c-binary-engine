//! Command-boundary evidence only. Never inspects M4 state or failed M5 outputs.
use serde::Serialize;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Serialize)]
pub(crate) struct Report {
    pub report_version: u8,
    pub command: String,
    pub mode: Option<&'static str>,
    pub status: &'static str,
    pub elapsed_ms: u128,
    pub error_category: Option<&'static str>,
    pub input_bytes: Option<u64>,
    pub output_bytes: Option<u64>,
    pub dataset_records: Option<u64>,
    pub dataset_parts: Option<u64>,
    pub batch_records: Option<usize>,
    pub record_length: Option<usize>,
    pub publication: Option<serde_json::Value>,
    pub warnings: Vec<&'static str>,
    #[serde(skip)]
    pub enabled: bool,
    #[serde(skip)]
    output: Option<PathBuf>,
}

fn file_size(path: &Path) -> Option<u64> {
    std::fs::metadata(path)
        .ok()
        .filter(|m| m.is_file())
        .map(|m| m.len())
}

impl Report {
    pub fn new(command: &str, enabled: bool) -> Self {
        Self {
            report_version: 1,
            command: command.into(),
            mode: None,
            status: "error",
            elapsed_ms: 0,
            error_category: Some("arguments"),
            input_bytes: None,
            output_bytes: None,
            dataset_records: None,
            dataset_parts: None,
            batch_records: None,
            record_length: None,
            publication: None,
            warnings: Vec::new(),
            enabled,
            output: None,
        }
    }
    pub fn files(&mut self, input: &Path, output: Option<&Path>) {
        if self.enabled {
            self.input_bytes = file_size(input);
            self.output = output.map(Path::to_owned);
        }
    }
    pub fn finish(&mut self, success: bool, elapsed: Duration) {
        self.elapsed_ms = elapsed.as_millis();
        if !success {
            return;
        }
        self.status = "success";
        self.error_category = None;
        if !self.enabled {
            return;
        }
        self.output_bytes = self.output.as_deref().and_then(file_size);
        if let (Some(bytes), Some(length)) = (self.input_bytes, self.record_length)
            && let Ok(length) = u64::try_from(length)
            && length != 0
            && bytes % length == 0
        {
            let rows = bytes / length;
            self.dataset_records = Some(rows);
            if self.command == "convert-parts"
                && let Some(batch) = self.batch_records.and_then(|b| u64::try_from(b).ok())
                && batch != 0
            {
                self.dataset_parts = Some(rows.div_ceil(batch).max(1));
            }
        }
    }
    pub fn write(&self, writer: &mut impl Write) -> io::Result<()> {
        serde_json::to_writer(&mut *writer, self).map_err(io::Error::other)?;
        writer.write_all(b"\n")?;
        writer.flush()
    }
    #[cfg(feature = "pqc")]
    pub fn protection(&mut self, outcome: &m2c_pipeline::protection::ProtectionOutcome) {
        self.publication = Some(serde_json::json!({"output": publication(&outcome.publication)}));
        self.warnings = outcome
            .warnings
            .iter()
            .map(|_| "permission_restriction_failed")
            .collect();
    }
    #[cfg(feature = "pqc")]
    pub fn keygen(&mut self, outcome: &m2c_pipeline::protection::KeyGenerationOutcome) {
        self.publication = Some(
            serde_json::json!({"public_key": publication(&outcome.public_key),
            "secret_key": publication(&outcome.secret_key)}),
        );
        self.warnings = outcome
            .warnings
            .iter()
            .map(|_| "permission_restriction_failed")
            .collect();
    }
}

#[cfg(feature = "pqc")]
fn publication(status: &m2c_pipeline::protection::PublicationStatus) -> &'static str {
    use m2c_pipeline::protection::PublicationStatus;
    match status {
        PublicationStatus::Published => "published",
        PublicationStatus::PublishedWithStagingResidue(_) => "published_with_staging_residue",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failure_has_no_publication_even_after_a_partial_keygen() {
        // Reporter receives only Err, never a path or a reconstructed outcome.
        let mut report = Report::new("keygen", true);
        report.error_category = Some("protection");
        report.finish(false, Duration::from_millis(42));
        let value = serde_json::to_value(&report).unwrap();
        assert_eq!(value["publication"], serde_json::Value::Null);
        assert_eq!(value["status"], "error");
        assert_eq!(value["elapsed_ms"], 42);
        assert_eq!(value["warnings"], serde_json::json!([]));
    }
    #[test]
    fn broken_report_writer_is_an_error_and_does_not_change_success() {
        struct Broken;
        impl Write for Broken {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::ErrorKind::BrokenPipe.into())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let mut report = Report::new("convert-parts", true);
        report.input_bytes = Some(105);
        report.record_length = Some(35);
        report.batch_records = Some(2);
        report.finish(true, Duration::ZERO);
        assert!(report.write(&mut Broken).is_err());
        assert_eq!(report.status, "success");
        assert_eq!(report.dataset_parts, Some(2));
    }
}

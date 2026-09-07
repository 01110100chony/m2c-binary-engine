//! Creation of a single local Parquet output without overwriting existing files.

use crate::ConversionError;
use arrow_schema::SchemaRef;
use parquet::arrow::ArrowWriter;
use parquet::basic::Compression;
use parquet::file::properties::WriterProperties;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;

pub(crate) fn create_writer(
    output: &Path,
    schema: SchemaRef,
    batch_records: usize,
) -> Result<ArrowWriter<File>, ConversionError> {
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .map_err(|source| ConversionError::Io {
            operation: "create output",
            path: output.to_owned(),
            source,
        })?;
    writer_from(file, schema, batch_records)
}

/// Share M3's writer settings while allowing M4 to keep ownership of the file
/// until finalization and synchronization have both succeeded.
pub(crate) fn writer_from<W: Write + Send>(
    sink: W,
    schema: SchemaRef,
    batch_records: usize,
) -> Result<ArrowWriter<W>, ConversionError> {
    let properties = WriterProperties::builder()
        .set_compression(Compression::UNCOMPRESSED)
        .set_max_row_group_size(batch_records)
        .build();
    Ok(ArrowWriter::try_new(sink, schema, Some(properties))?)
}

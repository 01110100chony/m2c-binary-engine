//! Minimal synchronous file-to-file orchestration.

use crate::{CompiledCopybook, DecodeError, RecordDecoder, parquet_io, source};
use parquet::errors::ParquetError;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Conversion failures preserve the underlying I/O, decoder or Parquet cause.
#[derive(Debug)]
pub enum ConversionError {
    InvalidBatchSize,
    CapacityExceeded,
    EmptySchema,
    Io {
        operation: &'static str,
        path: PathBuf,
        source: io::Error,
    },
    Decode {
        /// Zero-based batch start in the file. Source byte offsets remain batch-relative;
        /// source record indices are translated by M3 to global file indices.
        batch_offset: u64,
        source: DecodeError,
    },
    TruncatedRecord {
        /// Zero-based start of the incomplete record in the input file.
        byte_offset: u64,
        actual: usize,
        record_length: usize,
    },
    Parquet(ParquetError),
}
impl Display for ConversionError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidBatchSize => write!(f, "batch-records must be greater than zero"),
            Self::CapacityExceeded => {
                write!(f, "conversion buffer capacity or file offset exceeded")
            }
            Self::EmptySchema => write!(
                f,
                "Parquet conversion requires at least one non-FILLER field"
            ),
            Self::Io {
                operation,
                path,
                source,
            } => write!(f, "{operation} {}: {source}", path.display()),
            Self::Decode {
                batch_offset,
                source,
            } => {
                let relative = source
                    .context
                    .as_ref()
                    .map_or(0, |context| context.byte_offset);
                write!(
                    f,
                    "input file byte {}: {source}",
                    u128::from(*batch_offset) + relative as u128
                )
            }
            Self::TruncatedRecord {
                byte_offset,
                actual,
                record_length,
            } => write!(
                f,
                "incomplete record at input file byte {byte_offset}: expected {record_length} bytes, got {actual}"
            ),
            Self::Parquet(source) => write!(f, "Parquet output: {source}"),
        }
    }
}
impl Error for ConversionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Decode { source, .. } => Some(source),
            Self::Parquet(source) => Some(source),
            _ => None,
        }
    }
}
impl From<ParquetError> for ConversionError {
    fn from(source: ParquetError) -> Self {
        Self::Parquet(source)
    }
}

/// Convert fixed records using an already compiled layout into a new Parquet file.
///
/// At most `batch_records` records are decoded at a time. Each batch is flushed
/// as a row group; Parquet footer metadata grows with the number of row groups.
/// Empty inputs preserve the schema. Layouts containing only FILLER are rejected.
/// Existing outputs are never overwritten. On failure, a partial output may remain;
/// this operation provides neither atomic commit nor recovery.
pub fn convert_file(
    layout: &CompiledCopybook,
    input: &Path,
    output: &Path,
    batch_records: usize,
) -> Result<(), ConversionError> {
    let decoder = RecordDecoder::try_new(layout).map_err(|source| ConversionError::Decode {
        batch_offset: 0,
        source,
    })?;
    if layout.arrow_schema.fields().is_empty() {
        return Err(ConversionError::EmptySchema);
    }
    let mut buffer = source::batch_buffer(layout.record_length, batch_records)?;
    let mut file = File::open(input).map_err(|source| ConversionError::Io {
        operation: "open input",
        path: input.to_owned(),
        source,
    })?;
    let mut writer =
        parquet_io::create_writer(output, Arc::new(layout.arrow_schema.clone()), batch_records)?;
    let mut batch_offset = 0_u64;
    loop {
        let length =
            source::read_batch(&mut file, &mut buffer).map_err(|source| ConversionError::Io {
                operation: "read input",
                path: input.to_owned(),
                source,
            })?;
        if length == 0 {
            break;
        }
        let next_offset = batch_offset
            .checked_add(u64::try_from(length).map_err(|_| ConversionError::CapacityExceeded)?)
            .ok_or(ConversionError::CapacityExceeded)?;
        let remaining = length % layout.record_length;
        if remaining != 0 {
            return Err(ConversionError::TruncatedRecord {
                byte_offset: next_offset - remaining as u64,
                actual: remaining,
                record_length: layout.record_length,
            });
        }
        let batch = match decoder.decode_batch(&buffer[..length]) {
            Ok(batch) => batch,
            Err(mut source) => {
                if let Some(context) = source.context.as_mut() {
                    let record_length = u64::try_from(layout.record_length)
                        .map_err(|_| ConversionError::CapacityExceeded)?;
                    let first_record = usize::try_from(batch_offset / record_length)
                        .map_err(|_| ConversionError::CapacityExceeded)?;
                    context.record_index = first_record
                        .checked_add(context.record_index)
                        .ok_or(ConversionError::CapacityExceeded)?;
                }
                return Err(ConversionError::Decode {
                    batch_offset,
                    source,
                });
            }
        };
        writer.write(&batch)?;
        writer.flush()?;
        batch_offset = next_offset;
    }
    writer.close()?;
    Ok(())
}

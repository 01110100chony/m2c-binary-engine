use thiserror::Error;

#[derive(Debug, Error)]
pub enum MainframeError {
    #[error("invalid EBCDIC byte at offset {offset}: 0x{byte:02X}")]
    InvalidEbcdicByte { offset: usize, byte: u8 },
    #[error("invalid COMP-3 packed decimal in field {field}")]
    InvalidComp3 { field: String },
    #[error("record length mismatch: expected {expected}, got {actual}")]
    RecordLengthMismatch { expected: usize, actual: usize },
    #[error("endianness conversion failed for field {field}")]
    EndiannessMismatch { field: String },
    #[error("copybook schema error: {details}")]
    CopybookSchemaError { details: String },
    #[error("parquet serialization error: {details}")]
    ParquetSerializationError { details: String },
    #[error("post-quantum key exchange failed: {details}")]
    PqcHandshakeFailed { details: String },
    #[error("azure blob upload failed: {details}")]
    CloudSinkError { details: String },
    #[error("prometheus metrics export failed: {details}")]
    TelemetryExportError { details: String },
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
}

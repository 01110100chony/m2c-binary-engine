//! Core library for the experimental M2C pipeline.
//!
//! Copybooks are compiled once (M1), then fixed-record batches are decoded into
//! Arrow arrays (M2). Local files are converted synchronously to Parquet in
//! bounded batches (M3), with additive deterministic parts and process-interruption
//! recovery (M4). Cryptography and cloud belong to later milestones.

mod codec;
pub mod copybook;
pub mod decode;
pub mod error;
mod manifest;
mod parquet_io;
mod pipeline;
mod recovery;
pub mod schema;
mod source;

pub use copybook::{
    CopybookAst, DataEntry, EntryKind, Picture, PictureKind, Usage, normalize_fixed_format,
    parse_copybook,
};
pub use decode::RecordDecoder;
pub use error::{
    CopybookDiagnostic, DecodeContext, DecodeError, DecodeErrorKind, DiagnosticKind, SourceSpan,
};
pub use pipeline::{ConversionError, convert_file};
pub use recovery::{RecoveryError, RecoveryMode, convert_parts};
pub use schema::{
    CompiledCopybook, CompiledField, LogicalType, PhysicalEncoding, compile_copybook,
    parse_and_compile_copybook,
};

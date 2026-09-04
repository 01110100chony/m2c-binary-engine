//! Core library for the experimental M2C pipeline.
//!
//! Copybooks are compiled once (M1), then fixed-record batches are decoded into
//! Arrow arrays (M2). File sources, Parquet, cryptography, and sinks belong to
//! later milestones.

mod codec;
pub mod copybook;
pub mod decode;
pub mod error;
pub mod schema;

pub use copybook::{
    CopybookAst, DataEntry, EntryKind, Picture, PictureKind, Usage, normalize_fixed_format,
    parse_copybook,
};
pub use decode::RecordDecoder;
pub use error::{
    CopybookDiagnostic, DecodeContext, DecodeError, DecodeErrorKind, DiagnosticKind, SourceSpan,
};
pub use schema::{
    CompiledCopybook, CompiledField, LogicalType, PhysicalEncoding, compile_copybook,
    parse_and_compile_copybook,
};

//! Core library for the experimental M2C pipeline.
//!
//! Milestone M1 intentionally contains only the copybook front-end and layout
//! compiler. Record decoding, Parquet writing, cryptography, and sinks belong to
//! later milestones.

pub mod copybook;
pub mod error;
pub mod schema;

pub use copybook::{
    CopybookAst, DataEntry, EntryKind, Picture, PictureKind, Usage, normalize_fixed_format,
    parse_copybook,
};
pub use error::{CopybookDiagnostic, DiagnosticKind, SourceSpan};
pub use schema::{
    CompiledCopybook, CompiledField, LogicalType, PhysicalEncoding, compile_copybook,
    parse_and_compile_copybook,
};

mod ast;
mod normalize;
mod parser;

pub use crate::error::{CopybookDiagnostic, DiagnosticKind, SourceSpan};
pub use ast::{CopybookAst, DataEntry, EntryKind, Picture, PictureKind, Usage};
pub use normalize::{NormalizedCopybook, NormalizedLine, normalize_fixed_format};
pub use parser::parse_copybook;

pub(crate) fn is_valid_data_name(value: &str) -> bool {
    value
        .bytes()
        .next()
        .is_some_and(|byte| byte.is_ascii_alphabetic())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        && !value.ends_with('-')
}

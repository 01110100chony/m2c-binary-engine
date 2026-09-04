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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_data_name_valid() {
        let valid_names = vec![
            "A",
            "DATA-NAME",
            "A-1-B",
            "A1",
            "VALID-NAME-1",
        ];

        for name in valid_names {
            assert!(
                is_valid_data_name(name),
                "Expected '{}' to be a valid data name",
                name
            );
        }
    }

    #[test]
    fn test_is_valid_data_name_invalid() {
        let invalid_names = vec![
            "",
            "1-DATA",
            "DATA-",
            "-DATA",
            "DATA_NAME",
            "DATA NAME",
        ];

        for name in invalid_names {
            assert!(
                !is_valid_data_name(name),
                "Expected '{}' to be an invalid data name",
                name
            );
        }
    }
}

use crate::error::{CopybookDiagnostic, DiagnosticKind, SourceSpan};

const INDICATOR_COLUMN: usize = 7;
const CODE_START_COLUMN: usize = 8;
const CODE_END_COLUMN: usize = 72;

/// One normalized code-area line and its location in the original source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedLine {
    pub text: String,
    pub source_line: usize,
    pub source_column: usize,
}

/// Fixed-format source after sequence fields and comments have been removed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedCopybook {
    pub lines: Vec<NormalizedLine>,
}

/// Normalize an ASCII COBOL fixed-format source.
///
/// Columns 1-6 are ignored, column 7 is interpreted as the indicator area,
/// and only columns 8-72 enter the parser. Positions continue to refer to the
/// original input.
pub fn normalize_fixed_format(source: &str) -> Result<NormalizedCopybook, CopybookDiagnostic> {
    let mut normalized_lines = Vec::new();

    for (line_index, line) in source.lines().enumerate() {
        let line_number = line_index + 1;

        if let Some((column, character)) = line
            .char_indices()
            .find(|(_, character)| !character.is_ascii())
        {
            return Err(CopybookDiagnostic::new(
                SourceSpan::new(line_number, column + 1),
                DiagnosticKind::InvalidSourceFormat {
                    details: format!("non-ASCII character {character:?} is not supported"),
                },
            ));
        }

        if let Some(column) = line.bytes().position(|byte| byte == b'\t') {
            return Err(CopybookDiagnostic::new(
                SourceSpan::new(line_number, column + 1),
                DiagnosticKind::InvalidSourceFormat {
                    details: "tabs are not allowed because columns must be unambiguous".into(),
                },
            ));
        }

        if let Some((column, byte)) = line
            .bytes()
            .enumerate()
            .find(|(_, byte)| byte.is_ascii_control())
        {
            return Err(CopybookDiagnostic::new(
                SourceSpan::new(line_number, column + 1),
                DiagnosticKind::InvalidSourceFormat {
                    details: format!("ASCII control byte 0x{byte:02X} is not supported"),
                },
            ));
        }

        if line.bytes().all(|byte| byte == b' ') {
            continue;
        }

        if line.len() < INDICATOR_COLUMN {
            return Err(CopybookDiagnostic::new(
                SourceSpan::new(line_number, line.len() + 1),
                DiagnosticKind::InvalidSourceFormat {
                    details: format!(
                        "non-blank line has {} columns; at least {INDICATOR_COLUMN} are required",
                        line.len()
                    ),
                },
            ));
        }

        let indicator = line.as_bytes()[INDICATOR_COLUMN - 1] as char;
        match indicator {
            '*' | '/' => continue,
            ' ' => {}
            _ => {
                return Err(CopybookDiagnostic::new(
                    SourceSpan::new(line_number, INDICATOR_COLUMN),
                    DiagnosticKind::InvalidIndicator { indicator },
                ));
            }
        }

        let code_start = CODE_START_COLUMN - 1;
        let code_end = line.len().min(CODE_END_COLUMN);
        let mut code = if code_start < code_end {
            &line[code_start..code_end]
        } else {
            ""
        };

        if let Some(comment_start) = code.find("*>") {
            code = &code[..comment_start];
        }

        normalized_lines.push(NormalizedLine {
            text: code.to_owned(),
            source_line: line_number,
            source_column: CODE_START_COLUMN,
        });
    }

    Ok(NormalizedCopybook {
        lines: normalized_lines,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_only_code_area_and_original_position() {
        let source =
            "000100 01 RECORD.                                                        IGNORED";
        let normalized = normalize_fixed_format(source).unwrap();

        assert_eq!(normalized.lines.len(), 1);
        assert_eq!(normalized.lines[0].text.len(), 65);
        assert!(normalized.lines[0].text.starts_with("01 RECORD."));
        assert!(!normalized.lines[0].text.contains("IGNORED"));
        assert_eq!(normalized.lines[0].source_line, 1);
        assert_eq!(normalized.lines[0].source_column, 8);
    }

    #[test]
    fn removes_indicator_and_inline_comments() {
        let source = concat!(
            "000100* THIS IS A COMMENT\n",
            "000200/ THIS IS ALSO A COMMENT\n",
            "000300 01 RECORD. *> INLINE COMMENT\n",
        );
        let normalized = normalize_fixed_format(source).unwrap();

        assert_eq!(normalized.lines.len(), 1);
        assert_eq!(normalized.lines[0].text, "01 RECORD. ");
        assert_eq!(normalized.lines[0].source_line, 3);
    }

    #[test]
    fn permits_short_blank_lines_but_not_short_code_lines() {
        assert!(normalize_fixed_format("   \n       01 RECORD.").is_ok());

        let error = normalize_fixed_format("ABC").unwrap_err();
        assert!(matches!(
            error.kind,
            DiagnosticKind::InvalidSourceFormat { .. }
        ));
    }

    #[test]
    fn rejects_tabs_non_ascii_and_unsupported_indicators() {
        for source in ["\t      01 R.", "000000 01 CAFÉ."] {
            let error = normalize_fixed_format(source).unwrap_err();
            assert!(matches!(
                error.kind,
                DiagnosticKind::InvalidSourceFormat { .. }
            ));
        }

        for indicator in ['-', 'D', '0'] {
            let source = format!("000000{indicator}01 RECORD.");
            let error = normalize_fixed_format(&source).unwrap_err();
            assert_eq!(error.span, SourceSpan::new(1, 7));
            assert_eq!(error.kind, DiagnosticKind::InvalidIndicator { indicator });
        }
    }
}

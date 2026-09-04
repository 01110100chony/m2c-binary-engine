use std::error::Error;
use std::fmt::{self, Display, Formatter};

/// One-based location in the original, fixed-format copybook source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceSpan {
    pub line: usize,
    pub column: usize,
}

impl SourceSpan {
    pub const fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }
}

/// Structured causes shared by parsing and layout-compilation diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticKind {
    InvalidSourceFormat { details: String },
    InvalidIndicator { indicator: char },
    InvalidLevel { value: String },
    InvalidName { value: String },
    InvalidPicture { value: String, details: String },
    UnexpectedToken { found: String, expected: String },
    UnexpectedEnd { expected: String },
    UnsupportedClause { clause: String },
    MissingPeriod,
    InvalidHierarchy { details: String },
    InvalidPrecision { precision: usize, max: u8 },
    InvalidLength { length: usize, max: usize },
    UnsupportedCombination { details: String },
    LayoutOverflow,
    DuplicateField { path: String },
}

impl Display for DiagnosticKind {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSourceFormat { details } => {
                write!(formatter, "invalid fixed-format source: {details}")
            }
            Self::InvalidIndicator { indicator } => write!(
                formatter,
                "unsupported indicator {indicator:?} in column 7; expected a space, '*' or '/'"
            ),
            Self::InvalidLevel { value } => write!(
                formatter,
                "invalid level {value:?}; expected exactly two digits in the range 01..49"
            ),
            Self::InvalidName { value } => write!(
                formatter,
                "invalid data-name {value:?}; expected a letter followed by letters, digits or hyphens"
            ),
            Self::InvalidPicture { value, details } => {
                write!(formatter, "invalid PICTURE {value:?}: {details}")
            }
            Self::UnexpectedToken { found, expected } => {
                write!(formatter, "unexpected token {found:?}; expected {expected}")
            }
            Self::UnexpectedEnd { expected } => {
                write!(formatter, "unexpected end of copybook; expected {expected}")
            }
            Self::UnsupportedClause { clause } => {
                write!(formatter, "unsupported COBOL clause {clause:?}")
            }
            Self::MissingPeriod => write!(formatter, "missing period at end of data entry"),
            Self::InvalidHierarchy { details } => {
                write!(formatter, "invalid copybook hierarchy: {details}")
            }
            Self::InvalidPrecision { precision, max } => write!(
                formatter,
                "numeric precision {precision} is outside the supported range 1..={max}"
            ),
            Self::InvalidLength { length, max } => write!(
                formatter,
                "field byte length {length} is outside the supported range 1..={max}"
            ),
            Self::UnsupportedCombination { details } => {
                write!(
                    formatter,
                    "unsupported PICTURE/USAGE combination: {details}"
                )
            }
            Self::LayoutOverflow => {
                write!(formatter, "compiled record layout exceeds addressable size")
            }
            Self::DuplicateField { path } => {
                write!(formatter, "duplicate field path {path:?}")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopybookDiagnostic {
    pub span: SourceSpan,
    pub kind: DiagnosticKind,
}

impl CopybookDiagnostic {
    pub const fn new(span: SourceSpan, kind: DiagnosticKind) -> Self {
        Self { span, kind }
    }
}

impl Display for CopybookDiagnostic {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "line {}, column {}: {}",
            self.span.line, self.span.column, self.kind
        )
    }
}

impl Error for CopybookDiagnostic {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diagnostic_kind_display() {
        assert_eq!(
            DiagnosticKind::InvalidSourceFormat { details: "bad format".to_string() }.to_string(),
            "invalid fixed-format source: bad format"
        );
        assert_eq!(
            DiagnosticKind::InvalidIndicator { indicator: 'X' }.to_string(),
            "unsupported indicator 'X' in column 7; expected a space, '*' or '/'"
        );
        assert_eq!(
            DiagnosticKind::InvalidLevel { value: "A1".to_string() }.to_string(),
            "invalid level \"A1\"; expected exactly two digits in the range 01..49"
        );
        assert_eq!(
            DiagnosticKind::InvalidName { value: "1BAD".to_string() }.to_string(),
            "invalid data-name \"1BAD\"; expected a letter followed by letters, digits or hyphens"
        );
        assert_eq!(
            DiagnosticKind::InvalidPicture { value: "PIC X".to_string(), details: "syntax error".to_string() }.to_string(),
            "invalid PICTURE \"PIC X\": syntax error"
        );
        assert_eq!(
            DiagnosticKind::UnexpectedToken { found: "ABC".to_string(), expected: "XYZ".to_string() }.to_string(),
            "unexpected token \"ABC\"; expected XYZ"
        );
        assert_eq!(
            DiagnosticKind::UnexpectedEnd { expected: "EOF".to_string() }.to_string(),
            "unexpected end of copybook; expected EOF"
        );
        assert_eq!(
            DiagnosticKind::UnsupportedClause { clause: "COMP-3".to_string() }.to_string(),
            "unsupported COBOL clause \"COMP-3\""
        );
        assert_eq!(
            DiagnosticKind::MissingPeriod.to_string(),
            "missing period at end of data entry"
        );
        assert_eq!(
            DiagnosticKind::InvalidHierarchy { details: "bad parent".to_string() }.to_string(),
            "invalid copybook hierarchy: bad parent"
        );
        assert_eq!(
            DiagnosticKind::InvalidPrecision { precision: 20, max: 18 }.to_string(),
            "numeric precision 20 is outside the supported range 1..=18"
        );
        assert_eq!(
            DiagnosticKind::InvalidLength { length: 50, max: 32 }.to_string(),
            "field byte length 50 is outside the supported range 1..=32"
        );
        assert_eq!(
            DiagnosticKind::UnsupportedCombination { details: "PIC X COMP".to_string() }.to_string(),
            "unsupported PICTURE/USAGE combination: PIC X COMP"
        );
        assert_eq!(
            DiagnosticKind::LayoutOverflow.to_string(),
            "compiled record layout exceeds addressable size"
        );
        assert_eq!(
            DiagnosticKind::DuplicateField { path: "A.B".to_string() }.to_string(),
            "duplicate field path \"A.B\""
        );
    }

    #[test]
    fn test_copybook_diagnostic_display() {
        let diag = CopybookDiagnostic::new(
            SourceSpan::new(42, 12),
            DiagnosticKind::MissingPeriod,
        );
        assert_eq!(
            diag.to_string(),
            "line 42, column 12: missing period at end of data entry"
        );
    }
}

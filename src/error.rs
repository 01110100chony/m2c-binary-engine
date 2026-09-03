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

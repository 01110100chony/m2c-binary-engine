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

/// Location of a decoding failure. Record indices and byte offsets are zero-based.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodeContext {
    pub record_index: usize,
    pub field_path: String,
    /// Offset within the input batch, pointing at the offending byte when known.
    pub byte_offset: usize,
    /// Original one-based copybook source location.
    pub span: SourceSpan,
}

/// Structured causes for layout validation and binary decoding failures.
#[derive(Debug)]
pub enum DecodeErrorKind {
    InvalidLayout {
        field_index: Option<usize>,
        details: String,
    },
    InvalidBatchLength {
        actual: usize,
        record_length: usize,
    },
    InvalidFieldLength {
        expected: usize,
        actual: usize,
    },
    InvalidDisplayDigit {
        offset: usize,
        byte: u8,
    },
    InvalidPackedDigit {
        nibble_index: usize,
        nibble: u8,
    },
    InvalidPackedSign {
        offset: usize,
        nibble: u8,
        signed: bool,
    },
    InvalidPackedPadding {
        nibble: u8,
    },
    NumericOutOfRange {
        value: i128,
        precision: u8,
    },
    CapacityExceeded {
        resource: &'static str,
    },
    Arrow(arrow_schema::ArrowError),
}

impl Display for DecodeErrorKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLayout {
                field_index,
                details,
            } => write!(
                f,
                "invalid compiled layout (field {field_index:?}): {details}"
            ),
            Self::InvalidBatchLength {
                actual,
                record_length,
            } => write!(
                f,
                "batch length {actual} is not a multiple of record length {record_length}"
            ),
            Self::InvalidFieldLength { expected, actual } => {
                write!(f, "expected {expected} field bytes, got {actual}")
            }
            Self::InvalidDisplayDigit { offset, byte } => write!(
                f,
                "invalid DISPLAY digit 0x{byte:02X} at field byte {offset}"
            ),
            Self::InvalidPackedDigit {
                nibble_index,
                nibble,
            } => write!(
                f,
                "invalid packed digit 0x{nibble:X} at nibble {nibble_index}"
            ),
            Self::InvalidPackedSign { nibble, signed, .. } => {
                write!(f, "invalid packed sign 0x{nibble:X} for signed={signed}")
            }
            Self::InvalidPackedPadding { nibble } => {
                write!(f, "nonzero packed padding nibble 0x{nibble:X}")
            }
            Self::NumericOutOfRange { value, precision } => {
                write!(f, "unscaled value {value} exceeds precision {precision}")
            }
            Self::CapacityExceeded { resource } => write!(f, "capacity exceeded for {resource}"),
            Self::Arrow(error) => write!(f, "Arrow: {error}"),
        }
    }
}

/// A typed failure; no partial RecordBatch is returned on error.
#[derive(Debug)]
pub struct DecodeError {
    pub kind: DecodeErrorKind,
    pub context: Option<Box<DecodeContext>>,
}

impl DecodeError {
    pub(crate) fn new(kind: DecodeErrorKind) -> Self {
        Self {
            kind,
            context: None,
        }
    }

    pub(crate) fn invalid_layout(field_index: Option<usize>, details: impl Into<String>) -> Self {
        Self::new(DecodeErrorKind::InvalidLayout {
            field_index,
            details: details.into(),
        })
    }

    pub(crate) fn capacity(resource: &'static str) -> Self {
        Self::new(DecodeErrorKind::CapacityExceeded { resource })
    }
}

impl Display for DecodeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if let Some(context) = &self.context {
            write!(
                f,
                "record {}, field {}, batch byte {} (line {}, column {}): ",
                context.record_index,
                context.field_path,
                context.byte_offset,
                context.span.line,
                context.span.column
            )?;
        }
        Display::fmt(&self.kind, f)
    }
}

impl Error for DecodeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.kind {
            DecodeErrorKind::Arrow(error) => Some(error),
            _ => None,
        }
    }
}

impl From<arrow_schema::ArrowError> for DecodeError {
    fn from(error: arrow_schema::ArrowError) -> Self {
        Self::new(DecodeErrorKind::Arrow(error))
    }
}

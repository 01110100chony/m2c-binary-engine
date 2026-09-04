//! Compilation of a parsed copybook into an immutable physical and logical layout.

use std::collections::HashSet;

use arrow_schema::{DataType, Field, Schema};

use crate::copybook::{
    CopybookAst, CopybookDiagnostic, DiagnosticKind, EntryKind, Picture, PictureKind, SourceSpan,
    Usage, is_valid_data_name, parse_copybook,
};

const MAX_NUMERIC_PRECISION: u8 = 18;
const MAX_UTF8_FIELD_BYTES: usize = i32::MAX as usize;

/// Physical representation occupied by a field in a mainframe record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhysicalEncoding {
    EbcdicText,
    EbcdicDisplayNumeric,
    BigEndianBinary,
    PackedDecimal,
}

/// Logical value exposed to Arrow after decoding in a later milestone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogicalType {
    Utf8,
    Int64,
    Decimal128 { precision: u8, scale: i8 },
}

impl LogicalType {
    pub fn arrow_data_type(self) -> DataType {
        match self {
            Self::Utf8 => DataType::Utf8,
            Self::Int64 => DataType::Int64,
            Self::Decimal128 { precision, scale } => DataType::Decimal128(precision, scale),
        }
    }
}

/// A fully resolved elementary item in physical record order.
///
/// `path` is absent for `FILLER`. Filler items remain in this list because they
/// occupy bytes, but are deliberately omitted from [`CompiledCopybook::arrow_schema`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledField {
    pub path: Option<String>,
    pub source_name: String,
    pub offset: usize,
    pub byte_length: usize,
    pub physical_encoding: PhysicalEncoding,
    pub signed: bool,
    pub precision: Option<u8>,
    pub scale: Option<i8>,
    pub logical_type: LogicalType,
    pub span: SourceSpan,
}

/// Immutable layout consumed by the future record decoder hot path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledCopybook {
    pub name: String,
    pub record_length: usize,
    pub fields: Vec<CompiledField>,
    pub arrow_schema: Schema,
}

#[derive(Debug, Clone)]
struct GroupFrame {
    level: u8,
    name: String,
}

#[derive(Debug, Clone, Copy)]
struct ResolvedField {
    byte_length: usize,
    physical_encoding: PhysicalEncoding,
    signed: bool,
    precision: Option<u8>,
    scale: Option<i8>,
    logical_type: LogicalType,
}

/// Compile a syntactically valid AST into deterministic offsets and Arrow types.
pub fn compile_copybook(ast: &CopybookAst) -> Result<CompiledCopybook, CopybookDiagnostic> {
    let root = ast.entries.first().ok_or_else(|| {
        hierarchy_diagnostic(
            SourceSpan::new(1, 8),
            "copybook must contain one root level 01 group",
        )
    })?;

    if root.level != 1 {
        return Err(hierarchy_diagnostic(
            root.span,
            "first entry must be the root level 01 group",
        ));
    }
    if !matches!(root.entry, EntryKind::Group) {
        return Err(hierarchy_diagnostic(
            root.span,
            "root level 01 entry must be a group",
        ));
    }
    if !is_valid_data_name(&root.name) {
        return Err(CopybookDiagnostic::new(
            root.span,
            DiagnosticKind::InvalidName {
                value: root.name.clone(),
            },
        ));
    }
    if root.name.eq_ignore_ascii_case("FILLER") {
        return Err(combination_diagnostic(
            root.span,
            "FILLER is supported only for elementary items",
        ));
    }

    let root_name = canonical_name(&root.name);
    let mut groups = vec![GroupFrame {
        level: root.level,
        name: root_name.clone(),
    }];
    let mut previous = root;
    let mut offset = 0usize;
    let mut fields = Vec::new();
    let mut arrow_fields = Vec::new();
    let mut field_paths = HashSet::new();

    for entry in ast.entries.iter().skip(1) {
        if entry.level == 1 {
            return Err(hierarchy_diagnostic(
                entry.span,
                "multiple root level 01 entries are not supported",
            ));
        }
        if !(2..=49).contains(&entry.level) {
            return Err(hierarchy_diagnostic(
                entry.span,
                "non-root entries must use levels 02 through 49",
            ));
        }
        if !is_valid_data_name(&entry.name) {
            return Err(CopybookDiagnostic::new(
                entry.span,
                DiagnosticKind::InvalidName {
                    value: entry.name.clone(),
                },
            ));
        }

        if entry.level > previous.level && matches!(previous.entry, EntryKind::Elementary { .. }) {
            return Err(hierarchy_diagnostic(
                entry.span,
                "an elementary item cannot contain child entries",
            ));
        }

        while groups
            .last()
            .is_some_and(|group| group.level >= entry.level)
        {
            groups.pop();
        }
        if groups.is_empty() {
            return Err(hierarchy_diagnostic(
                entry.span,
                "entry is not nested under the root group",
            ));
        }

        match &entry.entry {
            EntryKind::Group => {
                if entry.name.eq_ignore_ascii_case("FILLER") {
                    return Err(combination_diagnostic(
                        entry.span,
                        "FILLER is supported only for elementary items",
                    ));
                }
                let group_name = canonical_name(&entry.name);
                let group_path_components: Vec<&str> = groups
                    .iter()
                    .map(|g| g.name.as_str())
                    .chain(std::iter::once(group_name.as_str()))
                    .collect();
                let group_path = group_path_components.join(".");
                if !field_paths.insert(group_path.clone()) {
                    return Err(CopybookDiagnostic::new(
                        entry.span,
                        DiagnosticKind::DuplicateField { path: group_path },
                    ));
                }
                groups.push(GroupFrame {
                    level: entry.level,
                    name: group_name,
                });
            }
            EntryKind::Elementary { picture, usage } => {
                let resolved = resolve_field(picture, *usage, entry.span)?;
                let next_offset = offset.checked_add(resolved.byte_length).ok_or_else(|| {
                    CopybookDiagnostic::new(entry.span, DiagnosticKind::LayoutOverflow)
                })?;
                if next_offset > isize::MAX as usize {
                    return Err(CopybookDiagnostic::new(
                        entry.span,
                        DiagnosticKind::LayoutOverflow,
                    ));
                }
                let is_filler = entry.name.eq_ignore_ascii_case("FILLER");
                let path = if is_filler {
                    None
                } else {
                    let mut components = groups
                        .iter()
                        .map(|group| group.name.as_str())
                        .collect::<Vec<_>>();
                    let field_name = canonical_name(&entry.name);
                    components.push(&field_name);
                    let path = components.join(".");
                    if !field_paths.insert(path.clone()) {
                        return Err(CopybookDiagnostic::new(
                            entry.span,
                            DiagnosticKind::DuplicateField { path },
                        ));
                    }
                    Some(path)
                };

                if let Some(path) = &path {
                    arrow_fields.push(Field::new(
                        path,
                        resolved.logical_type.arrow_data_type(),
                        false,
                    ));
                }

                fields.push(CompiledField {
                    path,
                    source_name: canonical_name(&entry.name),
                    offset,
                    byte_length: resolved.byte_length,
                    physical_encoding: resolved.physical_encoding,
                    signed: resolved.signed,
                    precision: resolved.precision,
                    scale: resolved.scale,
                    logical_type: resolved.logical_type,
                    span: entry.span,
                });
                offset = next_offset;
            }
        }

        previous = entry;
    }

    for (index, entry) in ast.entries.iter().enumerate() {
        if matches!(entry.entry, EntryKind::Group)
            && ast
                .entries
                .get(index + 1)
                .is_none_or(|next| next.level <= entry.level)
        {
            return Err(hierarchy_diagnostic(
                entry.span,
                format!(
                    "group {} must contain a subordinate entry leading to an elementary item",
                    entry.name
                ),
            ));
        }
    }

    if fields.is_empty() {
        return Err(hierarchy_diagnostic(
            root.span,
            "root group must contain at least one elementary item",
        ));
    }

    Ok(CompiledCopybook {
        name: root_name,
        record_length: offset,
        fields,
        arrow_schema: Schema::new(arrow_fields),
    })
}

/// Parse fixed-format source and compile it once into the future decoder contract.
pub fn parse_and_compile_copybook(source: &str) -> Result<CompiledCopybook, CopybookDiagnostic> {
    let ast = parse_copybook(source)?;
    compile_copybook(&ast)
}

fn resolve_field(
    picture: &Picture,
    usage: Usage,
    span: SourceSpan,
) -> Result<ResolvedField, CopybookDiagnostic> {
    match picture.kind {
        PictureKind::Alphanumeric { length } => {
            if picture.signed || usage != Usage::Display {
                return Err(combination_diagnostic(
                    span,
                    "alphanumeric PIC X items require unsigned DISPLAY usage",
                ));
            }
            if length == 0 {
                return Err(CopybookDiagnostic::new(
                    span,
                    DiagnosticKind::InvalidPicture {
                        value: "X(0)".to_owned(),
                        details: "repetition must be greater than zero".to_owned(),
                    },
                ));
            }
            if length > MAX_UTF8_FIELD_BYTES {
                return Err(CopybookDiagnostic::new(
                    span,
                    DiagnosticKind::InvalidLength {
                        length,
                        max: MAX_UTF8_FIELD_BYTES,
                    },
                ));
            }
            Ok(ResolvedField {
                byte_length: length,
                physical_encoding: PhysicalEncoding::EbcdicText,
                signed: false,
                precision: None,
                scale: None,
                logical_type: LogicalType::Utf8,
            })
        }
        PictureKind::Numeric {
            integer_digits,
            fractional_digits,
        } => {
            let precision = usize::from(integer_digits)
                .checked_add(usize::from(fractional_digits))
                .ok_or_else(|| CopybookDiagnostic::new(span, DiagnosticKind::LayoutOverflow))?;
            if integer_digits == 0 || precision == 0 || precision > MAX_NUMERIC_PRECISION.into() {
                return Err(CopybookDiagnostic::new(
                    span,
                    DiagnosticKind::InvalidPrecision {
                        precision,
                        max: MAX_NUMERIC_PRECISION,
                    },
                ));
            }

            let precision = precision as u8;
            let scale = fractional_digits as i8;
            let decimal = LogicalType::Decimal128 { precision, scale };

            match usage {
                Usage::Display => {
                    if picture.signed {
                        return Err(combination_diagnostic(
                            span,
                            "signed DISPLAY and overpunch are outside the v0.1 subset",
                        ));
                    }
                    Ok(ResolvedField {
                        byte_length: usize::from(precision),
                        physical_encoding: PhysicalEncoding::EbcdicDisplayNumeric,
                        signed: false,
                        precision: Some(precision),
                        scale: Some(scale),
                        logical_type: if scale == 0 {
                            LogicalType::Int64
                        } else {
                            decimal
                        },
                    })
                }
                Usage::Binary => {
                    let byte_length = binary_byte_length(precision).ok_or_else(|| {
                        CopybookDiagnostic::new(
                            span,
                            DiagnosticKind::InvalidPrecision {
                                precision: usize::from(precision),
                                max: MAX_NUMERIC_PRECISION,
                            },
                        )
                    })?;
                    Ok(ResolvedField {
                        byte_length,
                        physical_encoding: PhysicalEncoding::BigEndianBinary,
                        signed: picture.signed,
                        precision: Some(precision),
                        scale: Some(scale),
                        logical_type: if scale == 0 {
                            LogicalType::Int64
                        } else {
                            decimal
                        },
                    })
                }
                Usage::PackedDecimal => Ok(ResolvedField {
                    byte_length: (usize::from(precision) + 2) / 2,
                    physical_encoding: PhysicalEncoding::PackedDecimal,
                    signed: picture.signed,
                    precision: Some(precision),
                    scale: Some(scale),
                    logical_type: decimal,
                }),
            }
        }
    }
}

fn binary_byte_length(precision: u8) -> Option<usize> {
    match precision {
        1..=4 => Some(2),
        5..=9 => Some(4),
        10..=18 => Some(8),
        _ => None,
    }
}

fn canonical_name(name: &str) -> String {
    name.to_ascii_uppercase()
}

fn hierarchy_diagnostic(span: SourceSpan, details: impl Into<String>) -> CopybookDiagnostic {
    CopybookDiagnostic::new(
        span,
        DiagnosticKind::InvalidHierarchy {
            details: details.into(),
        },
    )
}

fn combination_diagnostic(span: SourceSpan, details: impl Into<String>) -> CopybookDiagnostic {
    CopybookDiagnostic::new(
        span,
        DiagnosticKind::UnsupportedCombination {
            details: details.into(),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span() -> SourceSpan {
        SourceSpan::new(1, 8)
    }

    fn numeric(integer_digits: u8, fractional_digits: u8, signed: bool) -> Picture {
        Picture {
            kind: PictureKind::Numeric {
                integer_digits,
                fractional_digits,
            },
            signed,
        }
    }

    #[test]
    fn ibm_binary_storage_boundaries_are_explicit() {
        for (precision, expected) in [(1, 2), (4, 2), (5, 4), (9, 4), (10, 8), (18, 8)] {
            let resolved = resolve_field(&numeric(precision, 0, false), Usage::Binary, span())
                .expect("supported precision");
            assert_eq!(resolved.byte_length, expected, "precision {precision}");
        }
    }

    #[test]
    fn packed_decimal_storage_includes_sign_nibble() {
        for (precision, expected) in [(1, 1), (2, 2), (3, 2), (4, 3), (18, 10)] {
            let resolved =
                resolve_field(&numeric(precision, 0, false), Usage::PackedDecimal, span())
                    .expect("supported precision");
            assert_eq!(resolved.byte_length, expected, "precision {precision}");
        }
    }

    #[test]
    fn signed_display_is_rejected() {
        let error = resolve_field(&numeric(4, 0, true), Usage::Display, span())
            .expect_err("signed DISPLAY is not in the subset");
        assert!(error.to_string().contains("signed DISPLAY"));
    }

    #[test]
    fn text_field_limit_matches_arrow_utf8_offsets() {
        let at_limit = Picture {
            kind: PictureKind::Alphanumeric {
                length: MAX_UTF8_FIELD_BYTES,
            },
            signed: false,
        };
        assert_eq!(
            resolve_field(&at_limit, Usage::Display, span())
                .expect("Arrow Utf8 boundary should compile")
                .byte_length,
            MAX_UTF8_FIELD_BYTES
        );

        let above_limit = Picture {
            kind: PictureKind::Alphanumeric {
                length: MAX_UTF8_FIELD_BYTES + 1,
            },
            signed: false,
        };
        let error = resolve_field(&above_limit, Usage::Display, span())
            .expect_err("a value larger than Arrow Utf8 offsets must be rejected");
        assert!(matches!(error.kind, DiagnosticKind::InvalidLength { .. }));
    }

    #[test]
    fn test_binary_byte_length() {
        assert_eq!(binary_byte_length(0), None);
        for precision in 1..=4 {
            assert_eq!(binary_byte_length(precision), Some(2));
        }
        for precision in 5..=9 {
            assert_eq!(binary_byte_length(precision), Some(4));
        }
        for precision in 10..=18 {
            assert_eq!(binary_byte_length(precision), Some(8));
        }
        assert_eq!(binary_byte_length(19), None);
        assert_eq!(binary_byte_length(255), None);
    }
}

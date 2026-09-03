use crate::error::SourceSpan;

/// Syntax tree for the deliberately small M2C copybook subset.
///
/// Entries remain in source order. Groups are represented by their level rather
/// than nested nodes so the layout compiler can validate hierarchy and compute
/// offsets in one deterministic pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopybookAst {
    pub entries: Vec<DataEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataEntry {
    pub level: u8,
    pub name: String,
    pub entry: EntryKind,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryKind {
    Group,
    Elementary { picture: Picture, usage: Usage },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picture {
    pub kind: PictureKind,
    pub signed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PictureKind {
    Alphanumeric {
        length: usize,
    },
    Numeric {
        integer_digits: u8,
        fractional_digits: u8,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Usage {
    Display,
    Binary,
    PackedDecimal,
}

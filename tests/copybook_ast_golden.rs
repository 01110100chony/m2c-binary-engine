use m2c_pipeline::copybook::{
    CopybookAst, DataEntry, EntryKind, Picture, PictureKind, SourceSpan, Usage,
};
use m2c_pipeline::parse_copybook;

const SAMPLE_COPYBOOK: &str = include_str!("fixtures/sample_fixed.cpy");

fn span(line: usize) -> SourceSpan {
    SourceSpan { line, column: 8 }
}

fn group(level: u8, name: &str, line: usize) -> DataEntry {
    DataEntry {
        level,
        name: name.to_owned(),
        entry: EntryKind::Group,
        span: span(line),
    }
}

fn text(level: u8, name: &str, length: usize, line: usize) -> DataEntry {
    DataEntry {
        level,
        name: name.to_owned(),
        entry: EntryKind::Elementary {
            picture: Picture {
                kind: PictureKind::Alphanumeric { length },
                signed: false,
            },
            usage: Usage::Display,
        },
        span: span(line),
    }
}

#[allow(clippy::too_many_arguments)]
fn numeric(
    level: u8,
    name: &str,
    integer_digits: u8,
    fractional_digits: u8,
    signed: bool,
    usage: Usage,
    line: usize,
) -> DataEntry {
    DataEntry {
        level,
        name: name.to_owned(),
        entry: EntryKind::Elementary {
            picture: Picture {
                kind: PictureKind::Numeric {
                    integer_digits,
                    fractional_digits,
                },
                signed,
            },
            usage,
        },
        span: span(line),
    }
}

#[test]
fn fixed_format_copybook_parses_to_expected_ast() {
    let actual = parse_copybook(SAMPLE_COPYBOOK).expect("the golden copybook should parse");

    let expected = CopybookAst {
        entries: vec![
            group(1, "SAMPLE-RECORD", 1),
            group(5, "HEADER-GROUP", 2),
            text(10, "CUSTOMER-NAME", 10, 4),
            text(10, "FILLER", 2, 5),
            numeric(10, "ACCOUNT-NUMBER", 4, 0, false, Usage::Display, 6),
            numeric(10, "INTEREST-RATE", 5, 2, false, Usage::Display, 7),
            numeric(10, "BALANCE-BIN", 4, 0, true, Usage::Binary, 8),
            numeric(10, "RATE-BIN", 5, 2, false, Usage::Binary, 9),
            numeric(10, "AMOUNT-PACKED", 7, 2, true, Usage::PackedDecimal, 10),
            text(10, "FILLER", 1, 11),
        ],
    };

    assert_eq!(actual, expected);
}

use std::panic::{AssertUnwindSafe, catch_unwind};

use m2c_pipeline::{
    CopybookAst, DataEntry, DiagnosticKind, EntryKind, Picture, PictureKind, SourceSpan, Usage,
    compile_copybook, parse_and_compile_copybook,
};

fn span(line: usize) -> SourceSpan {
    SourceSpan::new(line, 8)
}

fn group(level: u8, name: &str, line: usize) -> DataEntry {
    DataEntry {
        level,
        name: name.to_owned(),
        entry: EntryKind::Group,
        span: span(line),
    }
}

fn elementary(level: u8, name: &str, picture: Picture, usage: Usage, line: usize) -> DataEntry {
    DataEntry {
        level,
        name: name.to_owned(),
        entry: EntryKind::Elementary { picture, usage },
        span: span(line),
    }
}

fn text_picture(length: usize) -> Picture {
    Picture {
        kind: PictureKind::Alphanumeric { length },
        signed: false,
    }
}

fn numeric_picture(integer_digits: u8, fractional_digits: u8) -> Picture {
    Picture {
        kind: PictureKind::Numeric {
            integer_digits,
            fractional_digits,
        },
        signed: false,
    }
}

fn ast_with_field(picture: Picture, usage: Usage) -> CopybookAst {
    CopybookAst {
        entries: vec![
            group(1, "ROOT", 1),
            elementary(5, "FIELD", picture, usage, 2),
        ],
    }
}

fn line_with_reference_area(code: &str, ignored: &str) -> String {
    assert!(code.len() <= 65, "code must fit in columns 8 through 72");
    format!("000100 {code:<65}{ignored}\n")
}

fn expect_error<T, E>(result: Result<T, E>, message: &str) -> E {
    match result {
        Ok(_) => panic!("{message}"),
        Err(error) => error,
    }
}

#[test]
fn empty_and_comment_only_sources_return_errors_without_panicking() {
    let cases = [
        ("empty", ""),
        ("blank", "   \r\n      \r\n"),
        (
            "comment only",
            "000100* FIRST COMMENT\r\n000200/ SECOND COMMENT\r\n",
        ),
        ("empty code area", "000100 \r\n"),
    ];

    for (name, source) in cases {
        let outcome = catch_unwind(AssertUnwindSafe(|| parse_and_compile_copybook(source)));
        let result = outcome.unwrap_or_else(|_| panic!("{name} source caused a panic"));
        assert!(result.is_err(), "{name} source should be rejected");
    }
}

#[test]
fn crlf_input_compiles_with_original_source_positions() {
    let source = concat!(
        "000100 01 CRLF-RECORD.\r\n",
        "000200 05 TEXT-FIELD PIC X(3).\r\n",
        "000300 05 COUNT-FIELD PIC 9(4) COMP.\r\n",
    );

    let compiled = parse_and_compile_copybook(source).expect("CRLF is a supported line ending");

    assert_eq!(compiled.name, "CRLF-RECORD");
    assert_eq!(compiled.record_length, 5);
    assert_eq!(compiled.fields[0].span, SourceSpan::new(2, 8));
    assert_eq!(compiled.fields[1].span, SourceSpan::new(3, 8));
}

#[test]
fn columns_after_72_are_ignored_even_when_they_contain_invalid_cobol() {
    let source = format!(
        "{}{}",
        line_with_reference_area("01 REFERENCE-AREA-RECORD.", " OCCURS 999."),
        line_with_reference_area("05 VALUE-FIELD PIC X(4).", " REDEFINES @@@")
    );

    let compiled = parse_and_compile_copybook(&source)
        .expect("the reference area after column 72 must not reach the parser");

    assert_eq!(compiled.record_length, 4);
    assert_eq!(compiled.fields.len(), 1);
    assert_eq!(compiled.fields[0].source_name, "VALUE-FIELD");
}

#[test]
fn malformed_fixed_format_and_picture_inputs_never_panic_or_compile() {
    let cases = [
        ("non ASCII", "000100 01 CAFÉ-RECORD.\n"),
        ("tab", "000100\t01 TAB-RECORD.\n"),
        ("short nonblank line", "ABC\n"),
        ("continuation indicator", "000100-01 CONTINUED.\n"),
        ("debug indicator", "000100D01 DEBUG-LINE.\n"),
        ("numeric indicator", "000100001 BAD-INDICATOR.\n"),
        ("unknown indicator", "000100?01 BAD-INDICATOR.\n"),
        (
            "vertical-tab control",
            "000100 01 ROOT.\n000200 05\x0BFIELD PIC X.\n",
        ),
        (
            "form-feed control",
            "000100 01 ROOT.\n000200 05\x0CFIELD PIC X.\n",
        ),
        (
            "zero alphanumeric repetition",
            "000100 01 ROOT.\n000200 05 FIELD PIC X(0).\n",
        ),
        (
            "zero numeric repetition",
            "000100 01 ROOT.\n000200 05 FIELD PIC 9(0).\n",
        ),
        (
            "zero fractional repetition",
            "000100 01 ROOT.\n000200 05 FIELD PIC 9(2)V9(0).\n",
        ),
        (
            "huge repetition",
            "000100 01 ROOT.\n000200 05 FIELD PIC X(999999999999999999999999999999).\n",
        ),
        (
            "multiple decimal markers",
            "000100 01 ROOT.\n000200 05 FIELD PIC 9(2)V9(2)V9(2).\n",
        ),
        (
            "unknown punctuation in picture",
            "000100 01 ROOT.\n000200 05 FIELD PIC X(4),.\n",
        ),
        (
            "unknown punctuation after picture",
            "000100 01 ROOT.\n000200 05 FIELD PIC X(4) ;.\n",
        ),
        (
            "missing period",
            "000100 01 ROOT.\n000200 05 FIELD PIC X(4)\n",
        ),
    ];

    for (name, source) in cases {
        let outcome = catch_unwind(AssertUnwindSafe(|| parse_and_compile_copybook(source)));
        let result = outcome.unwrap_or_else(|_| panic!("{name} caused a panic"));
        assert!(result.is_err(), "{name} should be rejected, not compiled");
    }
}

#[test]
fn malformed_picture_diagnostics_are_explicit() {
    let cases = [
        "X(0)",
        "9(0)",
        "9(2)V9(0)",
        "X(999999999999999999999999999999)",
        "9(2)V9(2)V9(2)",
        "X(4),",
    ];

    for picture in cases {
        let source = format!("000100 01 ROOT.\n000200 05 FIELD PIC {picture}.\n");
        let error = expect_error(
            parse_and_compile_copybook(&source),
            &format!("PICTURE {picture:?} should be rejected"),
        );
        assert!(
            matches!(error.kind, DiagnosticKind::InvalidPicture { .. }),
            "PICTURE {picture:?} produced {error:?}"
        );
    }
}

#[test]
fn duplicate_fully_qualified_fields_are_rejected_case_insensitively() {
    let source = concat!(
        "000100 01 ROOT.\n",
        "000200 05 GROUP-A.\n",
        "000300 10 item PIC X.\n",
        "000400 10 ITEM PIC 9.\n",
    );

    let error = parse_and_compile_copybook(source).expect_err("duplicate path must be rejected");
    assert_eq!(error.span, SourceSpan::new(4, 8));
    assert_eq!(
        error.kind,
        DiagnosticKind::DuplicateField {
            path: "ROOT.GROUP-A.ITEM".to_owned(),
        }
    );
}

#[test]
fn group_filler_and_roots_without_fields_are_rejected() {
    let cases = [
        (
            "group FILLER",
            "000100 01 ROOT.\n000200 05 FILLER.\n000300 10 FIELD PIC X.\n",
            "FILLER",
        ),
        ("empty root", "000100 01 ROOT.\n", "elementary"),
        (
            "root containing groups only",
            "000100 01 ROOT.\n000200 05 EMPTY-GROUP.\n",
            "elementary",
        ),
    ];

    for (name, source, cause) in cases {
        let error = expect_error(
            parse_and_compile_copybook(source),
            &format!("{name} should be rejected"),
        );
        assert!(
            error.to_string().contains(cause),
            "{name} diagnostic `{error}` should mention `{cause}`"
        );
    }
}

#[test]
fn invalid_manually_constructed_asts_return_errors_without_panicking() {
    let cases = vec![
        (
            "zero numeric digits",
            ast_with_field(numeric_picture(0, 0), Usage::Display),
        ),
        (
            "zero integer digits",
            ast_with_field(numeric_picture(0, 1), Usage::PackedDecimal),
        ),
        (
            "fractional precision above maximum",
            ast_with_field(numeric_picture(1, 19), Usage::PackedDecimal),
        ),
        (
            "total precision above maximum",
            ast_with_field(numeric_picture(18, 1), Usage::Binary),
        ),
        (
            "zero alphanumeric length",
            ast_with_field(text_picture(0), Usage::Display),
        ),
        (
            "alphanumeric binary",
            ast_with_field(text_picture(4), Usage::Binary),
        ),
        (
            "alphanumeric packed decimal",
            ast_with_field(text_picture(4), Usage::PackedDecimal),
        ),
    ];

    for (name, ast) in cases {
        let outcome = catch_unwind(AssertUnwindSafe(|| compile_copybook(&ast)));
        let result = outcome.unwrap_or_else(|_| panic!("{name} AST caused a panic"));
        assert!(result.is_err(), "{name} AST should be rejected");
    }
}

#[test]
fn invalid_names_in_manually_constructed_asts_are_rejected() {
    let cases = vec![
        CopybookAst {
            entries: vec![
                group(1, "1ROOT", 1),
                elementary(5, "FIELD", text_picture(1), Usage::Display, 2),
            ],
        },
        CopybookAst {
            entries: vec![
                group(1, "ROOT", 1),
                elementary(5, "BAD.NAME", text_picture(1), Usage::Display, 2),
            ],
        },
        CopybookAst {
            entries: vec![
                group(1, "ROOT", 1),
                elementary(5, "TRAILING-", text_picture(1), Usage::Display, 2),
            ],
        },
    ];

    for ast in cases {
        let error = compile_copybook(&ast).expect_err("invalid AST name should be rejected");
        assert!(matches!(error.kind, DiagnosticKind::InvalidName { .. }));
    }
}

#[test]
fn text_larger_than_arrow_utf8_capacity_is_rejected() {
    let source = format!(
        "000100 01 ROOT.\n000200 05 FIELD PIC X({}).\n",
        i32::MAX as usize + 1
    );

    let error = parse_and_compile_copybook(&source)
        .expect_err("a PIC X larger than Arrow Utf8 capacity must be rejected");
    assert_eq!(error.span, SourceSpan::new(2, 8));
    assert!(matches!(error.kind, DiagnosticKind::InvalidLength { .. }));
}

#[test]
fn invalid_hierarchies_return_errors_without_panicking() {
    let cases = vec![
        ("empty AST", CopybookAst { entries: vec![] }),
        (
            "first entry is not level 01",
            CopybookAst {
                entries: vec![group(5, "NOT-A-ROOT", 1)],
            },
        ),
        (
            "root is elementary",
            CopybookAst {
                entries: vec![elementary(1, "ROOT", text_picture(1), Usage::Display, 1)],
            },
        ),
        (
            "multiple roots",
            CopybookAst {
                entries: vec![
                    group(1, "FIRST", 1),
                    elementary(5, "FIELD", text_picture(1), Usage::Display, 2),
                    group(1, "SECOND", 3),
                ],
            },
        ),
        (
            "child below elementary item",
            CopybookAst {
                entries: vec![
                    group(1, "ROOT", 1),
                    elementary(5, "PARENT", text_picture(1), Usage::Display, 2),
                    elementary(10, "CHILD", text_picture(1), Usage::Display, 3),
                ],
            },
        ),
        (
            "level zero escapes root",
            CopybookAst {
                entries: vec![
                    group(1, "ROOT", 1),
                    elementary(0, "OUTSIDE", text_picture(1), Usage::Display, 2),
                ],
            },
        ),
        (
            "level above supported subordinate range",
            CopybookAst {
                entries: vec![
                    group(1, "ROOT", 1),
                    elementary(50, "OUTSIDE", text_picture(1), Usage::Display, 2),
                ],
            },
        ),
        (
            "empty nested group",
            CopybookAst {
                entries: vec![
                    group(1, "ROOT", 1),
                    group(5, "EMPTY", 2),
                    elementary(5, "SIBLING", text_picture(1), Usage::Display, 3),
                ],
            },
        ),
    ];

    for (name, ast) in cases {
        let outcome = catch_unwind(AssertUnwindSafe(|| compile_copybook(&ast)));
        let result = outcome.unwrap_or_else(|_| panic!("{name} caused a panic"));
        let error = expect_error(result, &format!("{name} should be rejected"));
        assert!(
            matches!(error.kind, DiagnosticKind::InvalidHierarchy { .. }),
            "{name} produced {error:?}"
        );
    }
}

#[test]
fn hierarchy_level_transitions_produce_stable_paths_and_offsets() {
    let source = concat!(
        "000100 01 ROOT.\n",
        "000200 05 OUTER.\n",
        "000300 10 FIRST PIC X(2).\n",
        "000400 10 INNER.\n",
        "000500 15 SECOND PIC 9(5) COMP.\n",
        "000600 05 SIBLING.\n",
        "000700 49 THIRD PIC X.\n",
    );

    let compiled = parse_and_compile_copybook(source).expect("hierarchy should be valid");
    let paths_and_offsets: Vec<_> = compiled
        .fields
        .iter()
        .map(|field| (field.path.as_deref(), field.offset))
        .collect();

    assert_eq!(
        paths_and_offsets,
        vec![
            (Some("ROOT.OUTER.FIRST"), 0),
            (Some("ROOT.OUTER.INNER.SECOND"), 2),
            (Some("ROOT.SIBLING.THIRD"), 6),
        ]
    );
    assert_eq!(compiled.record_length, 7);
}

#[test]
fn duplicate_named_group_under_same_parent_is_rejected() {
    let source = concat!(
        "000100 01 ROOT.\n",
        "000200 05 GRP.\n",
        "000300    10 FIELD-A PIC X.\n",
        "000400 05 GRP.\n",
        "000500    10 FIELD-B PIC X.\n",
    );

    let error = expect_error(
        parse_and_compile_copybook(source),
        "two groups with the same qualified path must be rejected",
    );
    assert_eq!(
        error.kind,
        DiagnosticKind::DuplicateField {
            path: "ROOT.GRP".to_owned(),
        }
    );
    assert_eq!(error.span, SourceSpan::new(4, 8));
}

#[test]
fn same_group_name_under_different_parents_is_accepted() {
    let source = concat!(
        "000100 01 ROOT.\n",
        "000200 05 PARENT-A.\n",
        "000300    10 DETAIL.\n",
        "000400       15 VALUE-A PIC X.\n",
        "000500 05 PARENT-B.\n",
        "000600    10 DETAIL.\n",
        "000700       15 VALUE-B PIC X.\n",
    );

    let compiled = parse_and_compile_copybook(source)
        .expect("the same group name under different parents should compile");

    let paths: Vec<_> = compiled
        .fields
        .iter()
        .map(|f| f.path.as_deref().unwrap())
        .collect();
    assert_eq!(
        paths,
        vec![
            "ROOT.PARENT-A.DETAIL.VALUE-A",
            "ROOT.PARENT-B.DETAIL.VALUE-B",
        ]
    );
    assert_eq!(compiled.record_length, 2);
}

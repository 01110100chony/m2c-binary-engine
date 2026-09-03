use std::panic::{AssertUnwindSafe, catch_unwind};

use m2c_pipeline::parse_and_compile_copybook;

struct RejectionCase {
    name: &'static str,
    source: &'static str,
    line: usize,
    column: usize,
    cause_fragment: &'static str,
}

const REJECTION_CASES: &[RejectionCase] = &[
    RejectionCase {
        name: "occurs",
        source: "000100 01 SAMPLE-RECORD.\n000200 05 ITEMS OCCURS 3 TIMES.\n",
        line: 2,
        column: 17,
        cause_fragment: "OCCURS",
    },
    RejectionCase {
        name: "redefines",
        source: concat!(
            "000100 01 SAMPLE-RECORD.\n",
            "000200 05 ORIGINAL PIC X(4).\n",
            "000300 05 ALIAS REDEFINES ORIGINAL PIC X(4).\n",
        ),
        line: 3,
        column: 17,
        cause_fragment: "REDEFINES",
    },
    RejectionCase {
        name: "value",
        source: concat!(
            "000100 01 SAMPLE-RECORD.\n",
            "000200 05 STATUS-CODE PIC X VALUE 'A'.\n",
        ),
        line: 2,
        column: 29,
        cause_fragment: "VALUE",
    },
    RejectionCase {
        name: "copy",
        source: "000100 01 SAMPLE-RECORD.\n000200 COPY COMMON-FIELDS.\n",
        line: 2,
        column: 8,
        cause_fragment: "COPY",
    },
    RejectionCase {
        name: "comp-5",
        source: concat!(
            "000100 01 SAMPLE-RECORD.\n",
            "000200 05 COUNTER PIC S9(9) COMP-5.\n",
        ),
        line: 2,
        column: 29,
        cause_fragment: "COMP-5",
    },
    RejectionCase {
        name: "signed display",
        source: concat!(
            "000100 01 SAMPLE-RECORD.\n",
            "000200 05 SIGNED-TEXT PIC S9(4) DISPLAY.\n",
        ),
        line: 2,
        column: 8,
        cause_fragment: "DISPLAY",
    },
    RejectionCase {
        name: "level 88",
        source: concat!(
            "000100 01 SAMPLE-RECORD.\n",
            "000200 05 STATUS-CODE PIC X.\n",
            "000300 88 STATUS-OK VALUE 'Y'.\n",
        ),
        line: 3,
        column: 8,
        cause_fragment: "88",
    },
    RejectionCase {
        name: "second root",
        source: concat!(
            "000100 01 FIRST-RECORD.\n",
            "000200 05 FIRST-FIELD PIC X.\n",
            "000300 01 SECOND-RECORD.\n",
            "000400 05 SECOND-FIELD PIC X.\n",
        ),
        line: 3,
        column: 8,
        cause_fragment: "root",
    },
    RejectionCase {
        name: "child under elementary",
        source: concat!(
            "000100 01 SAMPLE-RECORD.\n",
            "000200 05 VALUE-FIELD PIC X(4).\n",
            "000300 10 ILLEGAL-CHILD PIC X.\n",
        ),
        line: 3,
        column: 8,
        cause_fragment: "elementary",
    },
    RejectionCase {
        name: "precision over Decimal128 policy",
        source: concat!(
            "000100 01 SAMPLE-RECORD.\n",
            "000200 05 TOO-WIDE PIC 9(17)V9(2) COMP-3.\n",
        ),
        line: 2,
        column: 8,
        cause_fragment: "precision",
    },
];

#[test]
fn unsupported_or_invalid_constructs_are_rejected_with_position_and_cause() {
    for case in REJECTION_CASES {
        let error = match parse_and_compile_copybook(case.source) {
            Ok(_) => panic!("{} should be rejected", case.name),
            Err(error) => error,
        };

        assert_eq!(error.span.line, case.line, "{} diagnostic line", case.name);
        assert_eq!(
            error.span.column, case.column,
            "{} diagnostic column",
            case.name
        );

        let rendered = error.to_string();
        assert!(
            rendered.contains(case.cause_fragment),
            "{} diagnostic `{rendered}` should mention `{}`",
            case.name,
            case.cause_fragment
        );
        assert!(
            rendered.contains(&format!("line {}, column {}", case.line, case.column)),
            "{} diagnostic `{rendered}` should contain its position",
            case.name
        );
    }
}

#[test]
fn malformed_fixed_format_inputs_return_errors_instead_of_panicking() {
    const MALFORMED: &[(&str, &str)] = &[
        ("missing entry", "000100 \n"),
        ("invalid indicator", "000100?01 SAMPLE-RECORD.\n"),
        ("invalid level", "000100 AA SAMPLE-RECORD.\n"),
        (
            "invalid picture",
            "000100 01 SAMPLE-RECORD.\n000200 05 BAD PIC 9(NOPE).\n",
        ),
        (
            "missing period",
            "000100 01 SAMPLE-RECORD.\n000200 05 FIELD PIC X(4)\n",
        ),
        (
            "unexpected tokens",
            "000100 01 SAMPLE-RECORD.\n000200 05 FIELD PIC X(4) GARBAGE.\n",
        ),
    ];

    for (name, source) in MALFORMED {
        let outcome = catch_unwind(AssertUnwindSafe(|| parse_and_compile_copybook(source)));
        let result = outcome.unwrap_or_else(|_| panic!("{name} caused a panic"));
        assert!(result.is_err(), "{name} should return an error");
    }
}

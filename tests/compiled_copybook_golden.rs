use arrow_schema::{DataType, Field, Schema};
use m2c_pipeline::copybook::SourceSpan;
use m2c_pipeline::{
    CompiledCopybook, LogicalType, PhysicalEncoding, compile_copybook, parse_and_compile_copybook,
    parse_copybook,
};

const SAMPLE_COPYBOOK: &str = include_str!("fixtures/sample_fixed.cpy");

struct ExpectedField {
    path: Option<&'static str>,
    source_name: &'static str,
    offset: usize,
    byte_length: usize,
    physical_encoding: PhysicalEncoding,
    signed: bool,
    precision: Option<u8>,
    scale: Option<i8>,
    logical_type: LogicalType,
    span: SourceSpan,
}

fn expected_fields() -> Vec<ExpectedField> {
    vec![
        ExpectedField {
            path: Some("SAMPLE-RECORD.HEADER-GROUP.CUSTOMER-NAME"),
            source_name: "CUSTOMER-NAME",
            offset: 0,
            byte_length: 10,
            physical_encoding: PhysicalEncoding::EbcdicText,
            signed: false,
            precision: None,
            scale: None,
            logical_type: LogicalType::Utf8,
            span: SourceSpan { line: 4, column: 8 },
        },
        ExpectedField {
            path: None,
            source_name: "FILLER",
            offset: 10,
            byte_length: 2,
            physical_encoding: PhysicalEncoding::EbcdicText,
            signed: false,
            precision: None,
            scale: None,
            logical_type: LogicalType::Utf8,
            span: SourceSpan { line: 5, column: 8 },
        },
        ExpectedField {
            path: Some("SAMPLE-RECORD.HEADER-GROUP.ACCOUNT-NUMBER"),
            source_name: "ACCOUNT-NUMBER",
            offset: 12,
            byte_length: 4,
            physical_encoding: PhysicalEncoding::EbcdicDisplayNumeric,
            signed: false,
            precision: Some(4),
            scale: Some(0),
            logical_type: LogicalType::Int64,
            span: SourceSpan { line: 6, column: 8 },
        },
        ExpectedField {
            path: Some("SAMPLE-RECORD.HEADER-GROUP.INTEREST-RATE"),
            source_name: "INTEREST-RATE",
            offset: 16,
            byte_length: 7,
            physical_encoding: PhysicalEncoding::EbcdicDisplayNumeric,
            signed: false,
            precision: Some(7),
            scale: Some(2),
            logical_type: LogicalType::Decimal128 {
                precision: 7,
                scale: 2,
            },
            span: SourceSpan { line: 7, column: 8 },
        },
        ExpectedField {
            path: Some("SAMPLE-RECORD.HEADER-GROUP.BALANCE-BIN"),
            source_name: "BALANCE-BIN",
            offset: 23,
            byte_length: 2,
            physical_encoding: PhysicalEncoding::BigEndianBinary,
            signed: true,
            precision: Some(4),
            scale: Some(0),
            logical_type: LogicalType::Int64,
            span: SourceSpan { line: 8, column: 8 },
        },
        ExpectedField {
            path: Some("SAMPLE-RECORD.HEADER-GROUP.RATE-BIN"),
            source_name: "RATE-BIN",
            offset: 25,
            byte_length: 4,
            physical_encoding: PhysicalEncoding::BigEndianBinary,
            signed: false,
            precision: Some(7),
            scale: Some(2),
            logical_type: LogicalType::Decimal128 {
                precision: 7,
                scale: 2,
            },
            span: SourceSpan { line: 9, column: 8 },
        },
        ExpectedField {
            path: Some("SAMPLE-RECORD.HEADER-GROUP.AMOUNT-PACKED"),
            source_name: "AMOUNT-PACKED",
            offset: 29,
            byte_length: 5,
            physical_encoding: PhysicalEncoding::PackedDecimal,
            signed: true,
            precision: Some(9),
            scale: Some(2),
            logical_type: LogicalType::Decimal128 {
                precision: 9,
                scale: 2,
            },
            span: SourceSpan {
                line: 10,
                column: 8,
            },
        },
        ExpectedField {
            path: None,
            source_name: "FILLER",
            offset: 34,
            byte_length: 1,
            physical_encoding: PhysicalEncoding::EbcdicText,
            signed: false,
            precision: None,
            scale: None,
            logical_type: LogicalType::Utf8,
            span: SourceSpan {
                line: 11,
                column: 8,
            },
        },
    ]
}

fn assert_golden_layout(actual: &CompiledCopybook) {
    assert_eq!(actual.name, "SAMPLE-RECORD");
    assert_eq!(actual.record_length, 35);

    let expected = expected_fields();
    assert_eq!(actual.fields.len(), expected.len());

    for (index, (actual, expected)) in actual.fields.iter().zip(&expected).enumerate() {
        assert_eq!(actual.path.as_deref(), expected.path, "field {index} path");
        assert_eq!(
            actual.source_name, expected.source_name,
            "field {index} source name"
        );
        assert_eq!(actual.offset, expected.offset, "field {index} offset");
        assert_eq!(
            actual.byte_length, expected.byte_length,
            "field {index} byte length"
        );
        assert_eq!(
            actual.physical_encoding, expected.physical_encoding,
            "field {index} physical encoding"
        );
        assert_eq!(actual.signed, expected.signed, "field {index} signedness");
        assert_eq!(
            actual.precision, expected.precision,
            "field {index} precision"
        );
        assert_eq!(actual.scale, expected.scale, "field {index} scale");
        assert_eq!(
            actual.logical_type, expected.logical_type,
            "field {index} logical type"
        );
        assert_eq!(actual.span, expected.span, "field {index} source span");
    }
}

#[test]
fn ast_compiles_to_expected_physical_layout() {
    let ast = parse_copybook(SAMPLE_COPYBOOK).expect("the golden copybook should parse");
    let compiled = compile_copybook(&ast).expect("the golden AST should compile");

    assert_golden_layout(&compiled);
}

#[test]
fn convenience_api_matches_the_two_stage_pipeline() {
    let compiled =
        parse_and_compile_copybook(SAMPLE_COPYBOOK).expect("the golden copybook should compile");

    assert_golden_layout(&compiled);
}

#[test]
fn arrow_schema_omits_filler_and_preserves_exact_logical_types() {
    let compiled =
        parse_and_compile_copybook(SAMPLE_COPYBOOK).expect("the golden copybook should compile");

    let expected = Schema::new(vec![
        Field::new(
            "SAMPLE-RECORD.HEADER-GROUP.CUSTOMER-NAME",
            DataType::Utf8,
            false,
        ),
        Field::new(
            "SAMPLE-RECORD.HEADER-GROUP.ACCOUNT-NUMBER",
            DataType::Int64,
            false,
        ),
        Field::new(
            "SAMPLE-RECORD.HEADER-GROUP.INTEREST-RATE",
            DataType::Decimal128(7, 2),
            false,
        ),
        Field::new(
            "SAMPLE-RECORD.HEADER-GROUP.BALANCE-BIN",
            DataType::Int64,
            false,
        ),
        Field::new(
            "SAMPLE-RECORD.HEADER-GROUP.RATE-BIN",
            DataType::Decimal128(7, 2),
            false,
        ),
        Field::new(
            "SAMPLE-RECORD.HEADER-GROUP.AMOUNT-PACKED",
            DataType::Decimal128(9, 2),
            false,
        ),
    ]);

    assert_eq!(compiled.arrow_schema, expected);
    assert!(
        compiled
            .arrow_schema
            .fields()
            .iter()
            .all(|field| !field.name().contains("FILLER"))
    );
}

#[test]
fn binary_storage_uses_ibm_precision_ranges() {
    const COPYBOOK: &str = concat!(
        "000100 01 BINARY-RANGES.\n",
        "000200 05 DIGIT-1 PIC 9 COMP.\n",
        "000300 05 DIGITS-4 PIC 9(4) BINARY.\n",
        "000400 05 DIGITS-5 PIC 9(5) COMP-4.\n",
        "000500 05 DIGITS-9 PIC 9(9) COMP.\n",
        "000600 05 DIGITS-10 PIC 9(10) BINARY.\n",
        "000700 05 DIGITS-18 PIC 9(18) COMP-4.\n",
    );

    let compiled =
        parse_and_compile_copybook(COPYBOOK).expect("all IBM binary ranges should compile");
    let offsets_and_lengths: Vec<_> = compiled
        .fields
        .iter()
        .map(|field| (field.offset, field.byte_length))
        .collect();

    assert_eq!(
        offsets_and_lengths,
        vec![(0, 2), (2, 2), (4, 4), (8, 4), (12, 8), (20, 8)]
    );
    assert_eq!(compiled.record_length, 28);
    assert!(
        compiled
            .fields
            .iter()
            .all(|field| field.logical_type == LogicalType::Int64)
    );
}

#[test]
fn packed_decimal_without_fraction_still_maps_to_decimal128() {
    const COPYBOOK: &str = concat!(
        "000100 01 PACKED-RECORD.\n",
        "000200 05 PACKED-WHOLE PIC 9(8) PACKED-DECIMAL.\n",
    );

    let compiled = parse_and_compile_copybook(COPYBOOK).expect("packed decimal should compile");
    let field = &compiled.fields[0];

    assert_eq!(field.byte_length, 5);
    assert_eq!(field.precision, Some(8));
    assert_eq!(field.scale, Some(0));
    assert_eq!(
        field.logical_type,
        LogicalType::Decimal128 {
            precision: 8,
            scale: 0,
        }
    );
    assert_eq!(
        compiled.arrow_schema.field(0).data_type(),
        &DataType::Decimal128(8, 0)
    );
}

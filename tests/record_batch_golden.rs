use std::sync::Arc;

use arrow_array::{Array, ArrayRef, Decimal128Array, Int64Array, RecordBatch, StringArray};
use arrow_schema::{DataType, Field, Schema};
use m2c_pipeline::{DecodeErrorKind, RecordDecoder, SourceSpan, parse_and_compile_copybook};

const COPYBOOK: &str = include_str!("fixtures/sample_fixed.cpy");
const BINARY: &[u8] = include_bytes!("fixtures/sample_fixed.bin");

fn expected_batch() -> RecordBatch {
    let schema = Schema::new(vec![
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
    let columns: Vec<ArrayRef> = vec![
        Arc::new(StringArray::from(vec![
            "ALICE     ",
            "José      ",
            "\0\u{85}\n¤[]    ",
        ])),
        Arc::new(Int64Array::from(vec![42, 9999, 0])),
        Arc::new(
            Decimal128Array::from(vec![12345, 9999999, 0])
                .with_precision_and_scale(7, 2)
                .unwrap(),
        ),
        Arc::new(Int64Array::from(vec![-123, 9999, 0])),
        Arc::new(
            Decimal128Array::from(vec![123456, 9999999, 0])
                .with_precision_and_scale(7, 2)
                .unwrap(),
        ),
        Arc::new(
            Decimal128Array::from(vec![123456789, -123, 0])
                .with_precision_and_scale(9, 2)
                .unwrap(),
        ),
    ];
    RecordBatch::try_new(Arc::new(schema), columns).unwrap()
}

#[test]
fn known_binary_records_produce_the_exact_expected_arrow_batch() {
    let compiled = parse_and_compile_copybook(COPYBOOK).unwrap();
    assert_eq!(BINARY.len(), 3 * 35);
    let batch = RecordDecoder::try_new(&compiled)
        .unwrap()
        .decode_batch(BINARY)
        .unwrap();
    assert_eq!(batch, expected_batch());
    assert_eq!(batch.schema().as_ref(), &compiled.arrow_schema);
    for column in batch.columns() {
        assert_eq!(column.null_count(), 0);
        column.to_data().validate_full().unwrap();
    }
}

#[test]
fn empty_input_and_all_filler_layouts_preserve_schema_and_row_count() {
    let compiled = parse_and_compile_copybook(COPYBOOK).unwrap();
    let empty = RecordDecoder::try_new(&compiled)
        .unwrap()
        .decode_batch(&[])
        .unwrap();
    assert_eq!(empty, expected_batch().slice(0, 0));
    let filler = parse_and_compile_copybook(
        "       01 ROOT.\n       05 FILLER PIC 9(2).\n       05 FILLER PIC 9(2) COMP-3.\n",
    )
    .unwrap();
    let decoder = RecordDecoder::try_new(&filler).unwrap();
    // FILLER content is intentionally not interpreted, including invalid numerics.
    let batch = decoder.decode_batch(&[0xFF; 12]).unwrap();
    assert_eq!(batch.num_rows(), 3);
    assert_eq!(batch.num_columns(), 0);
    assert_eq!(decoder.decode_batch(&[]).unwrap().num_rows(), 0);
    assert!(decoder.decode_batch(&[0]).is_err());
}

#[test]
fn every_partial_record_is_rejected_before_decoding() {
    let compiled = parse_and_compile_copybook(COPYBOOK).unwrap();
    let decoder = RecordDecoder::try_new(&compiled).unwrap();
    for length in 0..BINARY.len() {
        if length % 35 != 0 {
            let error = decoder.decode_batch(&BINARY[..length]).unwrap_err();
            assert!(
                matches!(error.kind, DecodeErrorKind::InvalidBatchLength { actual, record_length: 35 } if actual == length)
            );
            assert!(error.context.is_none());
        }
    }
    let mut extra_byte = BINARY.to_vec();
    extra_byte.push(0);
    assert!(decoder.decode_batch(&extra_byte).is_err());
}

#[test]
fn errors_identify_the_offending_byte_and_decoder_recovers() {
    let compiled = parse_and_compile_copybook(COPYBOOK).unwrap();
    let decoder = RecordDecoder::try_new(&compiled).unwrap();
    let mut bad = BINARY.to_vec();
    bad[35 + 13] = 0x40;
    let error = decoder.decode_batch(&bad).unwrap_err();
    assert!(matches!(
        error.kind,
        DecodeErrorKind::InvalidDisplayDigit {
            offset: 1,
            byte: 0x40
        }
    ));
    let context = error.context.as_ref().unwrap();
    assert_eq!(context.record_index, 1);
    assert_eq!(context.byte_offset, 48);
    assert_eq!(context.span, SourceSpan::new(6, 8));
    assert_eq!(
        context.field_path,
        "SAMPLE-RECORD.HEADER-GROUP.ACCOUNT-NUMBER"
    );
    assert!(error.to_string().contains("batch byte 48"));
    // A failure in an earlier record wins, even in a later column.
    bad[30] = 0xA4;
    let error = decoder.decode_batch(&bad).unwrap_err();
    assert!(matches!(
        error.kind,
        DecodeErrorKind::InvalidPackedDigit {
            nibble_index: 2,
            nibble: 0xA
        }
    ));
    assert_eq!(error.context.as_ref().unwrap().record_index, 0);
    assert_eq!(error.context.as_ref().unwrap().byte_offset, 30);
    assert_eq!(decoder.decode_batch(BINARY).unwrap(), expected_batch());
}

#[test]
fn maximum_precision_and_scale_are_exact_for_every_numeric_mapping() {
    let compiled = parse_and_compile_copybook(concat!(
        "       01 LIMITS.\n",
        "       05 DISPLAY-WHOLE PIC 9(18).\n",
        "       05 DISPLAY-SCALED PIC 9(1)V9(17).\n",
        "       05 BINARY-WHOLE PIC 9(18) BINARY.\n",
        "       05 BINARY-SCALED PIC S9(1)V9(17) COMP.\n",
        "       05 PACKED-WHOLE PIC 9(18) PACKED-DECIMAL.\n",
        "       05 PACKED-SCALED PIC S9(1)V9(17) COMP-3.\n",
    ))
    .unwrap();
    const MAX: i64 = 999_999_999_999_999_999;
    let mut bytes = vec![0xF9; 36];
    bytes.extend(MAX.to_be_bytes());
    bytes.extend((-MAX).to_be_bytes());
    bytes.extend([0x09, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x9F]);
    bytes.extend([0x09, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x9D]);
    let batch = RecordDecoder::try_new(&compiled)
        .unwrap()
        .decode_batch(&bytes)
        .unwrap();
    for column in [0, 2] {
        assert_eq!(
            batch
                .column(column)
                .as_any()
                .downcast_ref::<Int64Array>()
                .unwrap()
                .value(0),
            MAX
        );
    }
    for (column, scale, expected) in [
        (1, 17, i128::from(MAX)),
        (3, 17, -i128::from(MAX)),
        (4, 0, i128::from(MAX)),
        (5, 17, -i128::from(MAX)),
    ] {
        let array = batch
            .column(column)
            .as_any()
            .downcast_ref::<Decimal128Array>()
            .unwrap();
        assert_eq!(array.data_type(), &DataType::Decimal128(18, scale));
        assert_eq!(array.value(0), expected);
        array.to_data().validate_full().unwrap();
    }
}

#[test]
fn out_of_picture_binary_and_unsigned_packed_signs_are_rejected() {
    for (picture, bytes, error_type) in [
        ("9(4) COMP", vec![0xFF, 0xFF], "range"),
        ("S9(4) COMP", vec![0x80, 0], "range"),
        ("9(9) BINARY", u32::MAX.to_be_bytes().to_vec(), "range"),
        ("9(18) BINARY", u64::MAX.to_be_bytes().to_vec(), "range"),
        ("9(3) COMP-3", vec![0x12, 0x3D], "sign"),
        ("9(3) COMP-3", vec![0x00, 0x0D], "sign"),
        ("9(3) COMP-3", vec![0x12, 0x3C], "sign"),
        ("9(2) COMP-3", vec![0x11, 0x2F], "padding"),
    ] {
        let source = format!("       01 ROOT.\n       05 ITEM PIC {picture}.\n");
        let compiled = parse_and_compile_copybook(&source).unwrap();
        let error = RecordDecoder::try_new(&compiled)
            .unwrap()
            .decode_batch(&bytes)
            .unwrap_err();
        assert!(
            match error_type {
                "range" => matches!(error.kind, DecodeErrorKind::NumericOutOfRange { .. }),
                "sign" => matches!(error.kind, DecodeErrorKind::InvalidPackedSign { .. }),
                _ => matches!(error.kind, DecodeErrorKind::InvalidPackedPadding { .. }),
            },
            "{picture}: {error}"
        );
        assert_eq!(error.context.as_ref().unwrap().field_path, "ROOT.ITEM");
    }
}

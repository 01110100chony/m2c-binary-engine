use std::sync::Arc;

use arrow_schema::{DataType, Field, Schema};
use m2c_pipeline::{
    CompiledCopybook, DecodeErrorKind, LogicalType, PhysicalEncoding, RecordDecoder,
    parse_and_compile_copybook,
};

fn rejects(mutate: impl FnOnce(&mut CompiledCopybook)) {
    let mut layout = parse_and_compile_copybook(include_str!("fixtures/sample_fixed.cpy")).unwrap();
    mutate(&mut layout);
    let error = RecordDecoder::try_new(&layout)
        .expect_err("malformed public layout must be rejected before decoding");
    assert!(
        matches!(error.kind, DecodeErrorKind::InvalidLayout { .. }),
        "{error}"
    );
}

#[test]
fn rejects_zero_overflowing_noncontiguous_and_incomplete_layouts() {
    rejects(|c| c.record_length = 0);
    rejects(|c| c.record_length = usize::MAX);
    rejects(|c| c.record_length += 1);
    rejects(|c| c.record_length -= 1);
    rejects(|c| c.fields.clear());
    rejects(|c| c.fields[0].byte_length = 0);
    rejects(|c| c.fields[0].byte_length = usize::MAX);
    rejects(|c| c.fields[0].byte_length = i32::MAX as usize + 1);
    rejects(|c| c.fields[1].offset += 1);
    rejects(|c| c.fields[1].offset -= 1);
    rejects(|c| c.fields[1].offset = usize::MAX);
    rejects(|c| c.fields[1].byte_length = usize::MAX);
    rejects(|c| c.fields.swap(0, 1));
}

#[test]
fn rejects_inconsistent_numeric_and_text_metadata() {
    rejects(|c| c.fields[0].signed = true);
    rejects(|c| c.fields[0].precision = Some(1));
    rejects(|c| c.fields[0].scale = Some(0));
    rejects(|c| c.fields[0].logical_type = LogicalType::Int64);
    rejects(|c| c.fields[2].precision = None);
    rejects(|c| c.fields[2].precision = Some(0));
    rejects(|c| c.fields[2].precision = Some(19));
    rejects(|c| c.fields[2].precision = Some(255));
    rejects(|c| c.fields[2].scale = None);
    rejects(|c| c.fields[2].scale = Some(-1));
    rejects(|c| c.fields[2].scale = Some(4));
    rejects(|c| c.fields[2].scale = Some(127));
    rejects(|c| c.fields[2].signed = true);
    rejects(|c| c.fields[2].physical_encoding = PhysicalEncoding::PackedDecimal);
    rejects(|c| {
        c.fields[3].logical_type = LogicalType::Decimal128 {
            precision: 6,
            scale: 2,
        }
    });
    rejects(|c| c.fields[3].scale = Some(1));
    rejects(|c| c.fields[4].precision = Some(5));
    rejects(|c| c.fields[6].physical_encoding = PhysicalEncoding::BigEndianBinary);
    // Structural validation includes FILLER even though its bytes are ignored.
    rejects(|c| c.fields[1].physical_encoding = PhysicalEncoding::EbcdicDisplayNumeric);
}

#[test]
fn rejects_inconsistent_names_filler_and_arrow_schema() {
    rejects(|c| c.name = "sample-record".to_owned());
    rejects(|c| c.name = "FILLER".to_owned());
    rejects(|c| c.fields[0].source_name = "1INVALID".to_owned());
    rejects(|c| c.fields[0].path = None);
    rejects(|c| c.fields[1].path = Some("SAMPLE-RECORD.FILLER".to_owned()));
    rejects(|c| c.fields[0].path = Some("OTHER.CUSTOMER-NAME".to_owned()));
    rejects(|c| c.fields[0].path = Some("SAMPLE-RECORD..CUSTOMER-NAME".to_owned()));
    rejects(|c| c.fields[0].path = Some("SAMPLE-RECORD".to_owned()));
    rejects(|c| c.fields[0].path = Some("SAMPLE-RECORD.HEADER-GROUP.WRONG-NAME".to_owned()));
    rejects(|c| c.fields[2].path = c.fields[0].path.clone());
    rejects(|c| c.arrow_schema = Schema::empty());
    rejects(|c| {
        let mut fields = c.arrow_schema.fields().to_vec();
        fields.swap(0, 1);
        c.arrow_schema = Schema::new(fields);
    });
    for field in [
        Field::new("WRONG-NAME", DataType::Utf8, false),
        Field::new(
            "SAMPLE-RECORD.HEADER-GROUP.CUSTOMER-NAME",
            DataType::Utf8,
            true,
        ),
        Field::new(
            "SAMPLE-RECORD.HEADER-GROUP.CUSTOMER-NAME",
            DataType::LargeUtf8,
            false,
        ),
    ] {
        rejects(|c| {
            let mut fields = c.arrow_schema.fields().to_vec();
            fields[0] = Arc::new(field);
            c.arrow_schema = Schema::new(fields);
        });
    }
    rejects(|c| {
        let mut fields = c.arrow_schema.fields().to_vec();
        fields.push(Arc::new(Field::new("EXTRA", DataType::Int64, false)));
        c.arrow_schema = Schema::new(fields);
    });
}

#[test]
fn arbitrary_two_byte_inputs_never_panic_and_respect_signedness() {
    let signed =
        parse_and_compile_copybook("       01 ROOT.\n       05 N PIC S9(4) BINARY.\n").unwrap();
    let unsigned =
        parse_and_compile_copybook("       01 ROOT.\n       05 N PIC 9(4) BINARY.\n").unwrap();
    let signed_decoder = RecordDecoder::try_new(&signed).unwrap();
    let unsigned_decoder = RecordDecoder::try_new(&unsigned).unwrap();
    for raw in 0..=u16::MAX {
        let bytes = raw.to_be_bytes();
        let signed_value = i16::from_be_bytes(bytes);
        assert_eq!(
            signed_decoder.decode_batch(&bytes).is_ok(),
            (-9999..=9999).contains(&signed_value)
        );
        assert_eq!(unsigned_decoder.decode_batch(&bytes).is_ok(), raw <= 9999);
    }
}

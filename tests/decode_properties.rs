use arrow_array::{Array, Decimal128Array, Int64Array};
use arrow_schema::DataType;
use m2c_pipeline::{LogicalType, PhysicalEncoding, RecordDecoder, parse_and_compile_copybook};
use proptest::prelude::*;
use proptest::test_runner::RngSeed;

proptest! {
    #![proptest_config(ProptestConfig {
        cases: std::env::var("M6_TEST_CASES").ok().map(|s| s.parse().expect("M6_TEST_CASES integer")).unwrap_or(256),
        rng_seed: RngSeed::Fixed(std::env::var("M6_TEST_SEED").ok().map(|s| s.parse().expect("M6_TEST_SEED integer")).unwrap_or(0x4D3243)),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    #[test]
    fn numeric_representations_preserve_values_and_scale(
        precision in 1_u8..=18, scale_seed in any::<u8>(),
        raw in any::<u64>(), negative in any::<bool>(),
    ) {
        let scale = scale_seed % precision;
        let pic = if scale == 0 { format!("9({precision})") }
            else { format!("9({})V9({scale})", precision - scale) };
        let source = format!("       01 ROOT.\n       05 D PIC {pic}.\n       05 B PIC S{pic} BINARY.\n       05 P PIC S{pic} COMP-3.\n");
        let compiled = parse_and_compile_copybook(&source).unwrap();
        let magnitude = raw % 10_u64.pow(u32::from(precision));
        let value = if negative { -(magnitude as i64) } else { magnitude as i64 };
        // Synthetic property inputs supplement the independent golden binary.
        let digits = format!("{magnitude:0width$}", width = usize::from(precision));
        let mut bytes: Vec<u8> = digits.bytes().map(|b| b - b'0' + 0xF0).collect();
        let width = match precision { 1..=4 => 2, 5..=9 => 4, _ => 8 };
        bytes.extend_from_slice(&value.to_be_bytes()[8 - width..]);
        let mut nibbles = Vec::new();
        if precision % 2 == 0 { nibbles.push(0); }
        nibbles.extend(digits.bytes().map(|b| b - b'0'));
        nibbles.push(if negative { 0xD } else { 0xC });
        bytes.extend(nibbles.chunks_exact(2).map(|pair| pair[0] * 16 + pair[1]));
        let batch = RecordDecoder::try_new(&compiled).unwrap().decode_batch(&bytes).unwrap();
        for (index, expected) in [(0, i128::from(magnitude)), (1, i128::from(value)), (2, i128::from(value))] {
            let array = batch.column(index);
            if scale == 0 && index != 2 {
                prop_assert_eq!(array.as_any().downcast_ref::<Int64Array>().unwrap().value(0) as i128, expected);
            } else {
                prop_assert_eq!(array.data_type(), &DataType::Decimal128(precision, scale as i8));
                prop_assert_eq!(array.as_any().downcast_ref::<Decimal128Array>().unwrap().value(0), expected);
            }
            prop_assert_eq!(array.null_count(), 0);
            prop_assert!(array.to_data().validate_full().is_ok());
        }
    }

    #[test]
    fn arbitrary_bytes_across_all_encodings_return_valid_batches_or_typed_errors(
        bytes in prop::collection::vec(any::<u8>(), 0..200),
        precision in 1_u8..=18, kind in 0_u8..4, signed in any::<bool>(),
    ) {
        let declaration = match kind {
            0 => format!("X({precision})"),
            1 => format!("9({precision})"),
            2 => format!("{}9({precision}) BINARY", if signed { "S" } else { "" }),
            _ => format!("{}9({precision}) COMP-3", if signed { "S" } else { "" }),
        };
        let layout = parse_and_compile_copybook(&format!("       01 ROOT.\n       05 ITEM PIC {declaration}.\n")).unwrap();
        match RecordDecoder::try_new(&layout).unwrap().decode_batch(&bytes) {
            Ok(batch) => {
                prop_assert_eq!(batch.num_rows(), bytes.len() / layout.record_length);
                let schema = batch.schema();
                prop_assert_eq!(schema.as_ref(), &layout.arrow_schema);
                prop_assert_eq!(batch.column(0).null_count(), 0);
                prop_assert!(batch.column(0).to_data().validate_full().is_ok());
            }
            Err(error) => prop_assert!(!error.to_string().is_empty()),
        }
    }

    #[test]
    fn decoding_whole_and_partitioned_batches_is_equivalent(
        records in prop::collection::vec(0_usize..3, 0..40), split_seed in any::<usize>(),
    ) {
        let layout = parse_and_compile_copybook(include_str!("fixtures/sample_fixed.cpy")).unwrap();
        let decoder = RecordDecoder::try_new(&layout).unwrap();
        let fixture = include_bytes!("fixtures/sample_fixed.bin");
        let bytes: Vec<u8> = records.iter().flat_map(|&index| fixture[index * 35..(index + 1) * 35].iter().copied()).collect();
        let split = split_seed % (records.len() + 1);
        let whole = decoder.decode_batch(&bytes).unwrap();
        prop_assert_eq!(decoder.decode_batch(&bytes[..split * 35]).unwrap(), whole.slice(0, split));
        prop_assert_eq!(decoder.decode_batch(&bytes[split * 35..]).unwrap(), whole.slice(split, records.len() - split));
    }

    #[test]
    fn arbitrary_public_layout_metadata_never_panics(
        offset in any::<usize>(), length in any::<usize>(), total in any::<usize>(),
        precision in proptest::option::of(any::<u8>()), scale in proptest::option::of(any::<i8>()),
        signed in any::<bool>(), physical in 0_u8..4, logical in 0_u8..3,
    ) {
        let mut layout = parse_and_compile_copybook("       01 ROOT.\n       05 ITEM PIC 9.\n").unwrap();
        layout.record_length = total;
        let field = &mut layout.fields[0];
        field.offset = offset;
        field.byte_length = length;
        field.precision = precision;
        field.scale = scale;
        field.signed = signed;
        field.physical_encoding = match physical { 0 => PhysicalEncoding::EbcdicText, 1 => PhysicalEncoding::EbcdicDisplayNumeric, 2 => PhysicalEncoding::BigEndianBinary, _ => PhysicalEncoding::PackedDecimal };
        field.logical_type = match logical { 0 => LogicalType::Utf8, 1 => LogicalType::Int64, _ => LogicalType::Decimal128 { precision: precision.unwrap_or(0), scale: scale.unwrap_or(0) } };
        if let Ok(decoder) = RecordDecoder::try_new(&layout) {
            let _ = decoder.decode_batch(&[0_u8; 16]);
        }
    }
}

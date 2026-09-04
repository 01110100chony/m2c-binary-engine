use crate::error::{DecodeError, DecodeErrorKind};

fn limit(precision: u8) -> Result<i128, DecodeError> {
    if !(1..=18).contains(&precision) {
        return Err(DecodeError::invalid_layout(
            None,
            "precision must be in 1..=18",
        ));
    }
    Ok(10_i128.pow(u32::from(precision)))
}

fn require_length(bytes: &[u8], expected: usize) -> Result<(), DecodeError> {
    if bytes.len() != expected {
        return Err(DecodeError::new(DecodeErrorKind::InvalidFieldLength {
            expected,
            actual: bytes.len(),
        }));
    }
    Ok(())
}

fn in_range(value: i128, precision: u8, limit: i128) -> Result<i128, DecodeError> {
    if value <= -limit || value >= limit {
        return Err(DecodeError::new(DecodeErrorKind::NumericOutOfRange {
            value,
            precision,
        }));
    }
    Ok(value)
}

pub(crate) fn decode_display(bytes: &[u8], precision: u8) -> Result<i128, DecodeError> {
    let limit = limit(precision)?;
    require_length(bytes, usize::from(precision))?;
    let mut value = 0_i128;
    for (offset, &byte) in bytes.iter().enumerate() {
        if !(0xF0..=0xF9).contains(&byte) {
            return Err(DecodeError::new(DecodeErrorKind::InvalidDisplayDigit {
                offset,
                byte,
            }));
        }
        value = append_digit(value, byte - 0xF0)?;
    }
    in_range(value, precision, limit)
}

pub(crate) fn decode_binary(
    bytes: &[u8],
    precision: u8,
    signed: bool,
) -> Result<i128, DecodeError> {
    let limit = limit(precision)?;
    let length = match precision {
        1..=4 => 2,
        5..=9 => 4,
        _ => 8,
    };
    require_length(bytes, length)?;
    // Sign extension into i128 also preserves the full unsigned u64 range before
    // checking PIC precision; no narrowing or wrapping cast is involved.
    let negative = signed && bytes.first().is_some_and(|byte| byte & 0x80 != 0);
    let mut value = if negative { -1_i128 } else { 0_i128 };
    for &byte in bytes {
        value = value
            .checked_mul(256)
            .and_then(|v| v.checked_add(i128::from(byte)))
            .ok_or_else(|| DecodeError::capacity("binary integer"))?;
    }
    in_range(value, precision, limit)
}

pub(crate) fn decode_packed(
    bytes: &[u8],
    precision: u8,
    signed: bool,
) -> Result<i128, DecodeError> {
    let limit = limit(precision)?;
    require_length(bytes, (usize::from(precision) + 2) / 2)?;
    let mut value = 0_i128;
    let mut negative = false;
    for (byte_index, &byte) in bytes.iter().enumerate() {
        for (half, nibble) in [byte >> 4, byte & 0x0F].into_iter().enumerate() {
            let nibble_index = byte_index * 2 + half;
            if byte_index + 1 == bytes.len() && half == 1 {
                negative = match (signed, nibble) {
                    (true, 0xA | 0xC | 0xE | 0xF) | (false, 0xF) => false,
                    (true, 0xB | 0xD) => true,
                    _ => {
                        return Err(DecodeError::new(DecodeErrorKind::InvalidPackedSign {
                            offset: byte_index,
                            nibble,
                            signed,
                        }));
                    }
                };
            } else if nibble_index == 0 && precision.is_multiple_of(2) {
                if nibble != 0 {
                    return Err(DecodeError::new(DecodeErrorKind::InvalidPackedPadding {
                        nibble,
                    }));
                }
            } else {
                if nibble > 9 {
                    return Err(DecodeError::new(DecodeErrorKind::InvalidPackedDigit {
                        nibble_index,
                        nibble,
                    }));
                }
                value = append_digit(value, nibble)?;
            }
        }
    }
    in_range(if negative { -value } else { value }, precision, limit)
}

fn append_digit(value: i128, digit: u8) -> Result<i128, DecodeError> {
    value
        .checked_mul(10)
        .and_then(|v| v.checked_add(i128::from(digit)))
        .ok_or_else(|| DecodeError::capacity("decimal integer"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_is_strict_ebcdic_and_exact_at_18_digits() {
        assert_eq!(decode_display(&[0xF0, 0xF1, 0xF2, 0xF3], 4).unwrap(), 123);
        assert_eq!(
            decode_display(&[0xF9; 18], 18).unwrap(),
            999_999_999_999_999_999
        );
        for byte in 0..=255 {
            assert_eq!(
                decode_display(&[byte], 1).is_ok(),
                (0xF0..=0xF9).contains(&byte)
            );
        }
    }

    #[test]
    fn binary_uses_big_endian_twos_complement_and_pic_range() {
        assert_eq!(decode_binary(&[0x01, 0x02], 4, true).unwrap(), 258);
        assert_eq!(decode_binary(&[0xFF, 0x85], 4, true).unwrap(), -123);
        for (precision, length) in [(4, 2), (9, 4), (18, 8)] {
            assert_eq!(
                decode_binary(&vec![0xFF; length], precision, true).unwrap(),
                -1
            );
            assert!(matches!(
                decode_binary(&vec![0xFF; length], precision, false)
                    .unwrap_err()
                    .kind,
                DecodeErrorKind::NumericOutOfRange { .. }
            ));
        }
        assert_eq!(
            decode_binary(&[0x00, 0x00, 0x80, 0x00], 5, false).unwrap(),
            32768
        );
        assert!(decode_binary(&[0x27, 0x10], 4, false).is_err());
        assert!(decode_binary(&[0xD8, 0xF0], 4, true).is_err());
        assert!(decode_binary(&i64::MIN.to_be_bytes(), 18, true).is_err());
        assert_eq!(
            decode_binary(&999_999_999_999_999_999_i64.to_be_bytes(), 18, true).unwrap(),
            999_999_999_999_999_999
        );
    }

    #[test]
    fn packed_signs_digits_and_padding_are_explicit() {
        for sign in 0..=15 {
            let result = decode_packed(&[0x12, 0x30 | sign], 3, true);
            match sign {
                0xA | 0xC | 0xE | 0xF => assert_eq!(result.unwrap(), 123),
                0xB | 0xD => assert_eq!(result.unwrap(), -123),
                _ => assert!(matches!(
                    result.unwrap_err().kind,
                    DecodeErrorKind::InvalidPackedSign { .. }
                )),
            }
            assert_eq!(decode_packed(&[0x10 | sign], 1, false).is_ok(), sign == 0xF);
        }
        assert_eq!(decode_packed(&[0x00, 0x0D], 2, true).unwrap(), 0);
        assert_eq!(decode_packed(&[0x01, 0x2C], 2, true).unwrap(), 12);
        assert!(matches!(
            decode_packed(&[0x11, 0x2C], 2, true).unwrap_err().kind,
            DecodeErrorKind::InvalidPackedPadding { .. }
        ));
        assert!(matches!(
            decode_packed(&[0x1A, 0x2C], 3, true).unwrap_err().kind,
            DecodeErrorKind::InvalidPackedDigit {
                nibble_index: 1,
                ..
            }
        ));
        let max = [0x09, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x99, 0x9D];
        assert_eq!(
            decode_packed(&max, 18, true).unwrap(),
            -999_999_999_999_999_999
        );
    }

    #[test]
    fn malformed_lengths_and_precision_return_errors() {
        for precision in [0, 19, 255] {
            assert!(decode_display(&[], precision).is_err());
            assert!(decode_binary(&[], precision, false).is_err());
            assert!(decode_packed(&[], precision, true).is_err());
        }
        for length in [0, 1, 3, 5, 100] {
            assert!(decode_binary(&vec![0; length], 4, false).is_err());
        }
        assert!(decode_display(&[0xF1], 2).is_err());
        assert!(decode_packed(&[], 1, true).is_err());
        assert!(decode_packed(&[0x1C, 0], 1, true).is_err());
    }
}

//! Synchronous decoding of caller-sized fixed-record batches.

use std::collections::HashSet;
use std::sync::Arc;

use arrow_array::builder::{Decimal128Builder, Int64Builder, StringBuilder};
use arrow_array::{ArrayRef, RecordBatch, RecordBatchOptions};
use arrow_schema::SchemaRef;

use crate::codec::{decode_binary, decode_cp037, decode_display, decode_packed, utf8_length};
use crate::copybook::is_valid_data_name;
use crate::error::{DecodeContext, DecodeError, DecodeErrorKind};
use crate::schema::{CompiledCopybook, CompiledField, LogicalType, PhysicalEncoding};

/// Borrows a compiled layout and validates it once, before any record decoding.
///
/// The caller supplies bounded batches containing whole, concatenated records.
/// Calls are independent: a failed batch does not affect subsequent calls.
/// Text retains all CP037 spaces and controls; output columns are non-nullable.
///
/// ```
/// use m2c_pipeline::{parse_and_compile_copybook, RecordDecoder};
/// use arrow_array::Int64Array;
///
/// let layout = parse_and_compile_copybook(
///     "       01 ROOT.\n       05 COUNT-FIELD PIC 9(2).\n"
/// )?;
/// let decoder = RecordDecoder::try_new(&layout)?;
/// let batch = decoder.decode_batch(&[0xF1, 0xF2, 0xF0, 0xF3])?;
/// assert_eq!(batch.num_rows(), 2);
/// assert_eq!(batch.column(0).as_any().downcast_ref::<Int64Array>().unwrap().values(), &[12, 3]);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug)]
pub struct RecordDecoder<'a> {
    layout: &'a CompiledCopybook,
    schema: SchemaRef,
}

impl<'a> RecordDecoder<'a> {
    pub fn try_new(layout: &'a CompiledCopybook) -> Result<Self, DecodeError> {
        validate_layout(layout)?;
        Ok(Self {
            layout,
            schema: Arc::new(layout.arrow_schema.clone()),
        })
    }

    /// Decode one batch, returning the first failure in record/field order.
    ///
    /// An empty input produces zero rows with the compiled schema. A layout
    /// containing only FILLER produces zero columns with the correct row count.
    /// No partial batch is returned on error. Offsets in errors are batch-relative.
    pub fn decode_batch(&self, bytes: &[u8]) -> Result<RecordBatch, DecodeError> {
        let record_length = self.layout.record_length;
        if !bytes.len().is_multiple_of(record_length) {
            return Err(DecodeError::new(DecodeErrorKind::InvalidBatchLength {
                actual: bytes.len(),
                record_length,
            }));
        }
        let rows = bytes.len() / record_length;
        let mut columns = self
            .layout
            .fields
            .iter()
            .filter(|field| field.path.is_some())
            .map(|field| Column::new(field.logical_type, rows))
            .collect::<Result<Vec<_>, _>>()?;
        let mut scratch = String::new();
        for (record_index, record) in bytes.chunks_exact(record_length).enumerate() {
            // chunks_exact and the validated contiguous layout bound this arithmetic.
            let record_start = record_index * record_length;
            for (field, column) in self
                .layout
                .fields
                .iter()
                .filter(|field| field.path.is_some())
                .zip(&mut columns)
            {
                let field_bytes = record
                    .get(field.offset..field.offset + field.byte_length)
                    .ok_or_else(|| {
                        DecodeError::invalid_layout(None, "field falls outside record")
                    })?;
                column
                    .append(field, field_bytes, &mut scratch)
                    .map_err(|mut error| {
                        let relative = match &error.kind {
                            DecodeErrorKind::InvalidDisplayDigit { offset, .. }
                            | DecodeErrorKind::InvalidPackedSign { offset, .. } => *offset,
                            DecodeErrorKind::InvalidPackedDigit { nibble_index, .. } => {
                                nibble_index / 2
                            }
                            _ => 0,
                        };
                        error.context = Some(Box::new(DecodeContext {
                            record_index,
                            field_path: field
                                .path
                                .as_deref()
                                .unwrap_or(&field.source_name)
                                .to_owned(),
                            byte_offset: record_start + field.offset + relative,
                            span: field.span,
                        }));
                        error
                    })?;
            }
        }
        let arrays = columns.into_iter().map(Column::finish).collect();
        RecordBatch::try_new_with_options(
            Arc::clone(&self.schema),
            arrays,
            &RecordBatchOptions::new().with_row_count(Some(rows)),
        )
        .map_err(DecodeError::from)
    }
}

fn canonical_name(name: &str) -> bool {
    is_valid_data_name(name) && !name.bytes().any(|byte| byte.is_ascii_lowercase())
}

fn validate_layout(layout: &CompiledCopybook) -> Result<(), DecodeError> {
    if !canonical_name(&layout.name) || layout.name == "FILLER" {
        return Err(DecodeError::invalid_layout(
            None,
            "invalid canonical root name",
        ));
    }
    if layout.record_length == 0
        || layout.record_length > isize::MAX as usize
        || layout.fields.is_empty()
    {
        return Err(DecodeError::invalid_layout(
            None,
            "record must contain fields and have a positive addressable length",
        ));
    }
    let mut next_offset = 0_usize;
    let mut column_index = 0;
    let mut paths = HashSet::new();
    for (index, field) in layout.fields.iter().enumerate() {
        let invalid = |details| DecodeError::invalid_layout(Some(index), details);
        if field.offset != next_offset || field.byte_length == 0 {
            return Err(invalid(
                "field lengths must be positive and offsets contiguous",
            ));
        }
        next_offset = next_offset
            .checked_add(field.byte_length)
            .filter(|&end| end <= layout.record_length)
            .ok_or_else(|| invalid("field length overflows or exceeds record length"))?;
        if !canonical_name(&field.source_name)
            || (field.source_name == "FILLER") != field.path.is_none()
        {
            return Err(invalid("invalid field name or FILLER path"));
        }
        validate_field(field).map_err(invalid)?;
        if let Some(path) = &field.path {
            let mut components = path.split('.');
            if components.next() != Some(layout.name.as_str())
                || !components.clone().all(canonical_name)
                || components.next_back() != Some(field.source_name.as_str())
                || !paths.insert(path.as_str())
            {
                return Err(invalid(
                    "field path must be canonical, unique, and qualified by its root",
                ));
            }
            let arrow_field = layout
                .arrow_schema
                .fields()
                .get(column_index)
                .ok_or_else(|| invalid("missing Arrow field"))?;
            if arrow_field.name() != path
                || arrow_field.is_nullable()
                || arrow_field.data_type() != &field.logical_type.arrow_data_type()
            {
                return Err(invalid(
                    "Arrow name, type, order or nullability differs from compiled field",
                ));
            }
            column_index += 1;
        }
    }
    if next_offset != layout.record_length || column_index != layout.arrow_schema.fields().len() {
        return Err(DecodeError::invalid_layout(
            None,
            "record length or Arrow column count differs from layout",
        ));
    }
    Ok(())
}

fn validate_field(field: &CompiledField) -> Result<(), &'static str> {
    if field.physical_encoding == PhysicalEncoding::EbcdicText {
        if field.logical_type != LogicalType::Utf8
            || field.signed
            || field.precision.is_some()
            || field.scale.is_some()
            || field.byte_length > i32::MAX as usize
        {
            return Err("inconsistent CP037 text metadata");
        }
        return Ok(());
    }
    let precision = field.precision.ok_or("missing numeric precision")?;
    let scale = field.scale.ok_or("missing numeric scale")?;
    if !(1..=18).contains(&precision) || scale < 0 || scale >= precision as i8 {
        return Err("numeric precision/scale outside the M1 subset");
    }
    let expected_type = if scale == 0 && field.physical_encoding != PhysicalEncoding::PackedDecimal
    {
        LogicalType::Int64
    } else {
        LogicalType::Decimal128 { precision, scale }
    };
    let expected_length = match field.physical_encoding {
        PhysicalEncoding::EbcdicDisplayNumeric => {
            if field.signed {
                return Err("signed DISPLAY is unsupported");
            }
            usize::from(precision)
        }
        PhysicalEncoding::BigEndianBinary => match precision {
            1..=4 => 2,
            5..=9 => 4,
            _ => 8,
        },
        PhysicalEncoding::PackedDecimal => (usize::from(precision) + 2) / 2,
        PhysicalEncoding::EbcdicText => return Err("unexpected text encoding"),
    };
    if field.logical_type != expected_type || field.byte_length != expected_length {
        return Err("numeric physical length or logical type differs from the M1 mapping");
    }
    Ok(())
}

enum Column {
    Text(StringBuilder),
    Integer(Int64Builder),
    Decimal(Decimal128Builder),
}

impl Column {
    fn new(logical: LogicalType, rows: usize) -> Result<Self, DecodeError> {
        match logical {
            LogicalType::Utf8 => {
                check_buffer_capacity(rows, 4, 1)?;
                Ok(Self::Text(StringBuilder::with_capacity(rows, 0)))
            }
            LogicalType::Int64 => {
                check_buffer_capacity(rows, 8, 0)?;
                Ok(Self::Integer(Int64Builder::with_capacity(rows)))
            }
            LogicalType::Decimal128 { precision, scale } => {
                check_buffer_capacity(rows, 16, 0)?;
                Ok(Self::Decimal(
                    Decimal128Builder::with_capacity(rows)
                        .with_precision_and_scale(precision, scale)?,
                ))
            }
        }
    }

    fn append(
        &mut self,
        field: &CompiledField,
        bytes: &[u8],
        scratch: &mut String,
    ) -> Result<(), DecodeError> {
        if let Self::Text(builder) = self {
            let length = utf8_length(bytes)?;
            check_utf8_growth(builder.values_slice().len(), length)?;
            scratch.clear();
            scratch
                .try_reserve_exact(length)
                .map_err(|_| DecodeError::capacity("CP037 scratch string"))?;
            decode_cp037(bytes, scratch);
            builder.append_value(scratch.as_str());
            return Ok(());
        }
        let precision = field
            .precision
            .ok_or_else(|| DecodeError::invalid_layout(None, "missing numeric precision"))?;
        let value = match field.physical_encoding {
            PhysicalEncoding::EbcdicDisplayNumeric => decode_display(bytes, precision)?,
            PhysicalEncoding::BigEndianBinary => decode_binary(bytes, precision, field.signed)?,
            PhysicalEncoding::PackedDecimal => decode_packed(bytes, precision, field.signed)?,
            PhysicalEncoding::EbcdicText => {
                return Err(DecodeError::invalid_layout(
                    None,
                    "text field with numeric builder",
                ));
            }
        };
        match self {
            Self::Integer(builder) => builder.append_value(i64::try_from(value).map_err(|_| {
                DecodeError::new(DecodeErrorKind::NumericOutOfRange { value, precision })
            })?),
            Self::Decimal(builder) => builder.append_value(value),
            Self::Text(_) => {
                return Err(DecodeError::invalid_layout(
                    None,
                    "numeric field with text builder",
                ));
            }
        }
        Ok(())
    }

    fn finish(self) -> ArrayRef {
        match self {
            Self::Text(mut builder) => Arc::new(builder.finish()),
            Self::Integer(mut builder) => Arc::new(builder.finish()),
            Self::Decimal(mut builder) => Arc::new(builder.finish()),
        }
    }
}

fn check_buffer_capacity(rows: usize, width: usize, extra: usize) -> Result<(), DecodeError> {
    // Arrow rounds buffer allocations up to 64-byte alignment.
    rows.checked_add(extra)
        .and_then(|n| n.checked_mul(width))
        .filter(|&bytes| bytes <= (isize::MAX as usize & !63))
        .ok_or_else(|| DecodeError::capacity("Arrow column buffer"))?;
    Ok(())
}

fn check_utf8_growth(current: usize, additional: usize) -> Result<(), DecodeError> {
    let total = current
        .checked_add(additional)
        .filter(|&total| total <= i32::MAX as usize)
        .ok_or_else(|| DecodeError::capacity("Arrow Utf8 offsets"))?;
    // MutableBuffer can double its capacity on append. Leave room for that
    // growth as well as alignment, including on 32-bit targets.
    check_buffer_capacity(total, 2, 0)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrow_capacity_limits_are_checked_without_large_allocations() {
        let maximum = (i32::MAX as usize).min((isize::MAX as usize & !63) / 2);
        assert!(check_utf8_growth(maximum - 2, 2).is_ok());
        assert!(check_utf8_growth(maximum, 1).is_err());
        assert!(check_utf8_growth(i32::MAX as usize, 1).is_err());
        assert!(check_utf8_growth(usize::MAX, 1).is_err());
        assert!(check_buffer_capacity(usize::MAX, 4, 1).is_err());
        assert!(check_buffer_capacity(isize::MAX as usize / 8 + 1, 8, 0).is_err());
        assert!(check_buffer_capacity(16, 16, 0).is_ok());
        assert!(check_buffer_capacity(0, 4, 1).is_ok());
    }
}

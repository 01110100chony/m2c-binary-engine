//! Bounded local input buffers and synchronous reads.

use crate::ConversionError;
use std::io::{self, Read};

pub(crate) fn batch_buffer(
    record_length: usize,
    batch_records: usize,
) -> Result<Vec<u8>, ConversionError> {
    if batch_records == 0 {
        return Err(ConversionError::InvalidBatchSize);
    }
    let capacity = record_length
        .checked_mul(batch_records)
        .filter(|&size| size <= isize::MAX as usize)
        .ok_or(ConversionError::CapacityExceeded)?;
    let mut buffer = Vec::new();
    buffer
        .try_reserve_exact(capacity)
        .map_err(|_| ConversionError::CapacityExceeded)?;
    buffer.resize(capacity, 0);
    Ok(buffer)
}

/// Fill the bounded buffer or stop at EOF; a short read alone is not EOF.
pub(crate) fn read_batch(reader: &mut impl Read, buffer: &mut [u8]) -> io::Result<usize> {
    let mut filled = 0;
    while filled < buffer.len() {
        match reader.read(&mut buffer[filled..]) {
            Ok(0) => break,
            Ok(count) => filled += count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
    Ok(filled)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct ShortReader {
        input: io::Cursor<Vec<u8>>,
        interrupted: bool,
    }
    impl Read for ShortReader {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            self.interrupted = !self.interrupted;
            if self.interrupted {
                return Err(io::ErrorKind::Interrupted.into());
            }
            let length = buffer.len().min(3);
            self.input.read(&mut buffer[..length])
        }
    }
    #[test]
    fn short_and_interrupted_reads_preserve_batch_boundaries_and_final_bytes() {
        let mut reader = ShortReader {
            input: io::Cursor::new((0..19).collect()),
            interrupted: false,
        };
        let mut buffer = batch_buffer(4, 2).unwrap();
        let mut actual = Vec::new();
        for expected_length in [8, 8, 3, 0] {
            let length = read_batch(&mut reader, &mut buffer).unwrap();
            assert_eq!(length, expected_length);
            actual.extend_from_slice(&buffer[..length]);
        }
        assert_eq!(actual, (0..19).collect::<Vec<u8>>());
    }
    #[test]
    fn rejects_zero_and_unaddressable_batch_capacity_without_allocating() {
        assert!(matches!(
            batch_buffer(35, 0),
            Err(ConversionError::InvalidBatchSize)
        ));
        for (length, rows) in [(35, usize::MAX), (isize::MAX as usize, 2)] {
            assert!(matches!(
                batch_buffer(length, rows),
                Err(ConversionError::CapacityExceeded)
            ));
        }
    }
    #[test]
    fn read_errors_are_not_treated_as_eof() {
        struct BrokenReader;
        impl Read for BrokenReader {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                Err(io::ErrorKind::PermissionDenied.into())
            }
        }
        let error = read_batch(&mut BrokenReader, &mut [0; 8]).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    }
}

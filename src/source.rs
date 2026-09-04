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

    struct AdversarialReader {
        data: Vec<u8>,
        pos: usize,
        steps: Vec<isize>,
        step_idx: usize,
    }

    impl Read for AdversarialReader {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if self.step_idx >= self.steps.len() {
                let remaining = self.data.len() - self.pos;
                if remaining == 0 {
                    return Ok(0);
                }
                let to_read = buf.len().min(remaining);
                buf[..to_read].copy_from_slice(&self.data[self.pos..self.pos + to_read]);
                self.pos += to_read;
                return Ok(to_read);
            }

            let step = self.steps[self.step_idx];
            self.step_idx += 1;

            if step == -1 {
                return Err(io::ErrorKind::Interrupted.into());
            }

            if step == 0 {
                return Ok(0);
            }

            let to_read = (step as usize)
                .min(buf.len())
                .min(self.data.len() - self.pos);
            if to_read == 0 && self.pos >= self.data.len() {
                return Ok(0);
            }

            buf[..to_read].copy_from_slice(&self.data[self.pos..self.pos + to_read]);
            self.pos += to_read;
            Ok(to_read)
        }
    }

    fn simulate_pipeline(
        data: &[u8],
        steps: Vec<isize>,
        record_length: usize,
        batch_records: usize,
    ) -> (Vec<u8>, bool) {
        let mut reader = AdversarialReader {
            data: data.to_vec(),
            pos: 0,
            steps,
            step_idx: 0,
        };

        let mut buffer = batch_buffer(record_length, batch_records).unwrap();
        let mut actual_data = Vec::new();
        let mut truncated = false;

        loop {
            let length = read_batch(&mut reader, &mut buffer).unwrap();
            if length == 0 {
                break;
            }
            let remaining = length % record_length;
            if remaining != 0 {
                truncated = true;
                actual_data.extend_from_slice(&buffer[..length]);
                break;
            }
            actual_data.extend_from_slice(&buffer[..length]);
        }
        (actual_data, truncated)
    }

    #[test]
    fn adversarial_read_audit_shows_no_skipped_duplicated_or_stale_bytes() {
        let data: Vec<u8> = (0..100).collect();

        // 1. Normal read
        let (actual, truncated) = simulate_pipeline(&data, vec![], 10, 2);
        assert_eq!(actual, data);
        assert!(!truncated);

        // 2. Short reads
        let (actual, truncated) =
            simulate_pipeline(&data, vec![1, 2, 3, 4, 5, 10, 20, 50, 5], 10, 2);
        assert_eq!(actual, data);
        assert!(!truncated);

        // 3. Interrupted
        let (actual, truncated) = simulate_pipeline(&data, vec![10, -1, -1, 5, -1, 85], 10, 2);
        assert_eq!(actual, data);
        assert!(!truncated);

        // 4. EOF exactly on a record boundary
        let (actual, truncated) = simulate_pipeline(&data[..40], vec![15, 15, 10, 0], 10, 2);
        assert_eq!(actual, data[..40]);
        assert!(!truncated);

        // 5. EOF inside a record
        let (actual, truncated) = simulate_pipeline(&data[..45], vec![20, 20, 5, 0], 10, 2);
        assert_eq!(actual, data[..45]);
        assert!(truncated);

        // 6. Repeated short reads with Interrupted
        let (actual, truncated) =
            simulate_pipeline(&data[..10], vec![-1, 1, -1, 1, -1, 1, 0], 10, 2);
        assert_eq!(actual, data[..3]);
        assert!(truncated);
    }
}

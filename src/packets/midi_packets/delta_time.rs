use byteorder::ReadBytesExt;
use byteorder::WriteBytesExt;
use std::io::{Read, Write};

pub(crate) trait ReadDeltaTimeExt: Read {
    fn read_delta_time(&mut self) -> std::io::Result<u32>;
}

pub(crate) trait WriteDeltaTimeExt: std::io::Write {
    fn write_delta_time(&mut self, delta_time: u32) -> std::io::Result<usize>;
    fn delta_time_size(delta_time: u32) -> usize;
}

impl<R: Read> ReadDeltaTimeExt for R {
    fn read_delta_time(&mut self) -> std::io::Result<u32> {
        let mut delta_time = 0u32;
        loop {
            let byte = self.read_u8()?;
            delta_time = (delta_time << 7) | (byte & 0x7F) as u32;
            if byte & 0b1000_0000 == 0 {
                break;
            }
        }
        Ok(delta_time)
    }
}

impl<W: Write> WriteDeltaTimeExt for W {
    fn write_delta_time(&mut self, delta_time: u32) -> std::io::Result<usize> {
        let num_bytes = Self::delta_time_size(delta_time);
        let value_to_write = delta_time;

        for i in (0..num_bytes).rev() {
            let mut byte = ((value_to_write >> (i * 7)) & 0x7F) as u8;
            if i > 0 {
                byte |= 0x80; // Set the continuation bit
            }
            self.write_u8(byte)?;
        }
        Ok(num_bytes)
    }

    fn delta_time_size(delta_time: u32) -> usize {
        let mut size = 0;
        let mut value = delta_time;

        while value > 0 {
            size += 1;
            value >>= 7;
        }

        if size == 0 {
            size = 1; // At least one byte for zero
        }

        size
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    fn test_delta_time_rw(delta_time: u32, expected_bytes: &[u8]) {
        // Test writing
        let mut buffer: Vec<u8> = Vec::new();
        let bytes_written = buffer.write_delta_time(delta_time).unwrap();
        assert_eq!(bytes_written, expected_bytes.len());
        assert_eq!(buffer, expected_bytes);

        // Test reading
        let mut reader = Cursor::new(buffer);
        let read_dt = reader.read_delta_time().unwrap();
        assert_eq!(read_dt, delta_time);
    }

    #[test]
    fn test_delta_time_zero() {
        test_delta_time_rw(0, &[0x00]);
    }

    #[test]
    fn test_one_byte_delta_time() {
        // Max value for 1 byte: 0x7F
        test_delta_time_rw(0x7F, &[0x7F]);
        test_delta_time_rw(0x40, &[0x40]); // Arbitrary value
    }

    #[test]
    fn test_two_byte_delta_time() {
        // Min value for 2 bytes: 0x80
        test_delta_time_rw(0x80, &[0b1000_0001, 0b0000_0000]);
        // Max value for 2 bytes: 0x3FFF
        test_delta_time_rw(0x3FFF, &[0xFF, 0x7F]);
        // Arbitrary value
        test_delta_time_rw(0x2000, &[0xC0, 0x00]);
    }

    #[test]
    fn test_three_byte_delta_time() {
        // Min value for 3 bytes: 0x4000
        test_delta_time_rw(0x4000, &[0x81, 0x80, 0x00]);
        // Max value for 3 bytes: 0x1FFFFF
        test_delta_time_rw(0x1FFFFF, &[0xFF, 0xFF, 0x7F]);
        // Arbitrary value
        test_delta_time_rw(0x100000, &[0xC0, 0x80, 0x00]);
    }

    #[test]
    fn test_four_byte_delta_time() {
        // Min value for 4 bytes: 0x200000
        test_delta_time_rw(0x200000, &[0x81, 0x80, 0x80, 0x00]);
        // Max value for 4 bytes: 0x0FFFFFFF (MIDI spec max)
        test_delta_time_rw(0x0FFFFFFF, &[0xFF, 0xFF, 0xFF, 0x7F]);
        // Arbitrary value
        test_delta_time_rw(0x08000000, &[0xC0, 0x80, 0x80, 0x00]);
    }

    #[test]
    fn test_size_calculation() {
        assert_eq!(<Vec<u8> as WriteDeltaTimeExt>::delta_time_size(0), 1);
        assert_eq!(<Vec<u8> as WriteDeltaTimeExt>::delta_time_size(0x7F), 1);
        assert_eq!(<Vec<u8> as WriteDeltaTimeExt>::delta_time_size(0x80), 2);
        assert_eq!(<Vec<u8> as WriteDeltaTimeExt>::delta_time_size(0x3FFF), 2);
        assert_eq!(<Vec<u8> as WriteDeltaTimeExt>::delta_time_size(0x4000), 3);
        assert_eq!(<Vec<u8> as WriteDeltaTimeExt>::delta_time_size(0x1FFFFF), 3);
        assert_eq!(<Vec<u8> as WriteDeltaTimeExt>::delta_time_size(0x200000), 4);
        assert_eq!(<Vec<u8> as WriteDeltaTimeExt>::delta_time_size(0x0FFFFFFF), 4);
    }
}

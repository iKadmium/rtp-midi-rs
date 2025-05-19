use byteorder::ReadBytesExt;
use byteorder::WriteBytesExt;
use std::io::{Read, Write};

#[derive(Debug, Clone, PartialEq)]
pub struct DeltaTime {
    delta_time: u32,
}

impl DeltaTime {
    pub fn new(delta_time: u32) -> Self {
        DeltaTime { delta_time }
    }

    pub fn zero() -> Self {
        DeltaTime { delta_time: 0 }
    }

    pub fn size(&self) -> usize {
        let mut size = 0;
        let mut value = self.delta_time;

        while value > 0 {
            size += 1;
            value >>= 7;
        }

        if size == 0 {
            size = 1; // At least one byte for zero
        }

        size
    }

    pub fn write<W: Write>(&self, writer: &mut W) -> Result<usize, std::io::Error> {
        let num_bytes = self.size();
        let value_to_write = self.delta_time;

        for i in (0..num_bytes).rev() {
            // Iterate from num_bytes-1 down to 0
            let mut byte = ((value_to_write >> (i * 7)) & 0x7F) as u8;
            if i > 0 {
                // If this is not the last byte (MSB of value)
                byte |= 0x80; // Set the continuation bit
            }
            writer.write_u8(byte)?;
        }
        Ok(num_bytes)
    }

    pub fn read<R: Read>(reader: &mut R) -> Result<Self, std::io::Error> {
        let mut delta_time = 0u32;
        loop {
            let byte = reader.read_u8()?;
            delta_time = (delta_time << 7) | (byte & 0x7F) as u32;
            if byte & 0b1000_0000 == 0 {
                break;
            }
        }
        Ok(DeltaTime::new(delta_time))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn test_delta_time_rw(delta_time_val: u32, expected_bytes: &[u8]) {
        let dt = DeltaTime::new(delta_time_val);

        // Test writing
        let mut buffer: Vec<u8> = Vec::new();
        let bytes_written = dt.write(&mut buffer).unwrap();
        assert_eq!(bytes_written, expected_bytes.len());
        assert_eq!(buffer, expected_bytes);

        // Test reading
        let mut reader = Cursor::new(buffer);
        let read_dt = DeltaTime::read(&mut reader).unwrap();
        assert_eq!(read_dt, dt);
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
        assert_eq!(DeltaTime::new(0).size(), 1);
        assert_eq!(DeltaTime::new(0x7F).size(), 1);
        assert_eq!(DeltaTime::new(0x80).size(), 2);
        assert_eq!(DeltaTime::new(0x3FFF).size(), 2);
        assert_eq!(DeltaTime::new(0x4000).size(), 3);
        assert_eq!(DeltaTime::new(0x1FFFFF).size(), 3);
        assert_eq!(DeltaTime::new(0x200000).size(), 4);
        assert_eq!(DeltaTime::new(0x0FFFFFFF).size(), 4);
    }
}

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::{
    fmt,
    io::{Error, ErrorKind, Read, Write},
};

use super::control_packet::ControlPacket;

pub struct ClockSyncPacket {
    pub count: u8,
    pub timestamps: [u64; 3],
    pub sender_ssrc: u32,
}

impl ClockSyncPacket {
    pub const SIZE: usize = 36;

    pub fn new(count: u8, timestamps: [u64; 3], sender_ssrc: u32) -> Self {
        ClockSyncPacket {
            count,
            timestamps,
            sender_ssrc,
        }
    }

    pub fn read<R: Read>(reader: &mut R) -> std::io::Result<Self> {
        let sender_ssrc = reader.read_u32::<BigEndian>()?;
        let count = reader.read_u8()?;

        // Skip reserved bytes
        for _ in 0..3 {
            reader.read_u8()?;
        }

        let mut timestamps = [0; 3];

        for i in 0..3 {
            timestamps[i] = reader.read_u64::<BigEndian>()?;
        }

        Ok(ClockSyncPacket {
            count,
            timestamps,
            sender_ssrc,
        })
    }

    pub fn write<W: Write>(&self, writer: &mut W) -> std::io::Result<usize> {
        ControlPacket::write_header(writer, b"CK")?;
        writer.write_u32::<BigEndian>(self.sender_ssrc)?;
        writer.write_u8(self.count)?;
        writer.write_all(&[0, 0, 0])?; // Reserved bytes
        writer.write_u64::<BigEndian>(self.timestamps[0])?;
        writer.write_u64::<BigEndian>(self.timestamps[1])?;
        writer.write_u64::<BigEndian>(self.timestamps[2])?;
        Ok(Self::SIZE)
    }
}

impl fmt::Debug for ClockSyncPacket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClockSyncPacket")
            .field("count", &self.count)
            .field("timestamps", &self.timestamps)
            .field("sender_ssrc", &self.sender_ssrc)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_parse_control_packet_0() {
        let buffer = [
            0xFF, 0xFF, 0x43, 0x4B, //header
            0xF5, 0x19, 0xAE, 0xB9, //sender ssrc
            0x00, //count
            0x00, 0x00, 0x00, //reserved
            0x00, 0x00, 0x00, 0x00, 0x72, 0xD4, 0xC5, 0x8E, // timestamp 1
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // timestamp 2
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // timestamp 3
        ]; // Example buffer for a ClockSync packet

        let result = ControlPacket::parse(&buffer);
        assert!(result.is_ok());
        if let ControlPacket::ClockSync(packet) = result.unwrap() {
            assert_eq!(packet.count, 0);
            assert_eq!(packet.sender_ssrc, 4112101049);
            assert_eq!(packet.timestamps[0], 1926546830);
        } else {
            panic!("Expected ClockSync packet");
        }
    }

    #[test]
    fn test_parse_control_packet_2() {
        let buffer = [
            0xFF, 0xFF, 0x43, 0x4B, //header
            0xF5, 0x19, 0xAE, 0xB9, //sender ssrc
            0x02, //count
            0x00, 0x00, 0x00, //reserved
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x72, // timestamp 1
            0x00, 0x00, 0x00, 0x00, 0x04, 0x3D, 0xC7, 0xDF, // timestamp 2
            0x00, 0x00, 0x00, 0x00, 0x72, 0xD4, 0xC5, 0xCD, // timestamp 3
        ];
        let result = ControlPacket::parse(&buffer);
        assert!(result.is_ok());
        if let ControlPacket::ClockSync(packet) = result.unwrap() {
            assert_eq!(packet.count, 2);
            assert_eq!(packet.sender_ssrc, 4112101049);
            assert_eq!(packet.timestamps[0], 114);
            assert_eq!(packet.timestamps[1], 71157727);
            assert_eq!(packet.timestamps[2], 1926546893);
        } else {
            panic!("Expected ClockSync packet");
        }
    }
}

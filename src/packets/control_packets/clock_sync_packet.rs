use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::io::{Read, Write};

use super::control_packet::ControlPacket;

#[derive(Debug)]
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

        for i in timestamps.iter_mut() {
            *i = 0;
        }

        Ok(ClockSyncPacket::new(count, timestamps, sender_ssrc))
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

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buffer = Vec::with_capacity(Self::SIZE);
        self.write(&mut buffer).expect("Failed to write ClockSyncPacket");
        buffer
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_control_packet_0() {
        let buffer = [
            0xF5, 0x19, 0xAE, 0xB9, //sender ssrc
            0x00, //count
            0x00, 0x00, 0x00, //reserved
            0x00, 0x00, 0x00, 0x00, 0x72, 0xD4, 0xC5, 0x8E, // timestamp 1
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // timestamp 2
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // timestamp 3
        ]; // Example buffer for a ClockSync packet

        let mut cursor = std::io::Cursor::new(buffer);

        let result = ClockSyncPacket::read(&mut cursor);
        match result {
            Ok(packet) => {
                assert_eq!(packet.count, 0);
                assert_eq!(packet.sender_ssrc, 4112101049);
                assert_eq!(packet.timestamps[0], 1926546830);
            }
            Err(e) => panic!("Failed to read ClockSync packet: {}", e),
        };
    }

    #[test]
    fn test_read_control_packet_2() {
        let buffer = [
            0xF5, 0x19, 0xAE, 0xB9, //sender ssrc
            0x02, //count
            0x00, 0x00, 0x00, //reserved
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x72, // timestamp 1
            0x00, 0x00, 0x00, 0x00, 0x04, 0x3D, 0xC7, 0xDF, // timestamp 2
            0x00, 0x00, 0x00, 0x00, 0x72, 0xD4, 0xC5, 0xCD, // timestamp 3
        ];
        let mut cursor = std::io::Cursor::new(buffer);

        let result = ClockSyncPacket::read(&mut cursor);
        match result {
            Ok(packet) => {
                assert_eq!(packet.count, 2);
                assert_eq!(packet.sender_ssrc, 4112101049);
                assert_eq!(packet.timestamps[0], 114);
                assert_eq!(packet.timestamps[1], 71157727);
                assert_eq!(packet.timestamps[2], 1926546893);
            }
            Err(e) => panic!("Failed to read ClockSync packet: {}", e),
        };
    }

    #[test]
    fn test_write_control_packet() {
        let expected = [
            0xFF, 0xFF, 0x43, 0x4B, //header
            0xF5, 0x19, 0xAE, 0xB9, //sender ssrc
            0x02, //count
            0x00, 0x00, 0x00, //reserved
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, // timestamp 1
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, // timestamp 2
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03,
        ];
        let packet = ClockSyncPacket::new(2, [1, 2, 3], 4112101049);
        let mut buffer = Vec::new();
        let result = packet.write(&mut buffer);
        assert!(result.is_ok());
        assert_eq!(buffer.len(), ClockSyncPacket::SIZE);
        assert_eq!(buffer, expected);
    }

    #[test]
    fn test_new() {
        let packet = ClockSyncPacket::new(2, [1, 2, 3], 4112101049);
        assert_eq!(packet.count, 2);
        assert_eq!(packet.sender_ssrc, 4112101049);
        assert_eq!(packet.timestamps[0], 1);
        assert_eq!(packet.timestamps[1], 2);
        assert_eq!(packet.timestamps[2], 3);
    }
}

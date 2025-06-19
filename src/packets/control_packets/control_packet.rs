use std::io::{Cursor, Error, ErrorKind, Write};

use super::{
    clock_sync_packet::ClockSyncPacket,
    session_initiation_packet::SessionInitiationPacket, // Ensure ReadOptionalStringExt is in scope if SessionInitiationPacket::read relies on it being used on the reader directly.
};

const CONTROL_PACKET_HEADER: [u8; 2] = [255, 255];

#[derive(Debug)]
pub enum ControlPacket {
    ClockSync(ClockSyncPacket),
    SessionInitiation(SessionInitiationPacket),
}

impl ControlPacket {
    pub(crate) const HEADER_SIZE: usize = 4;

    pub fn from_be_bytes(buffer: &[u8]) -> std::io::Result<ControlPacket> {
        let mut reader = Cursor::new(&buffer[4..]);
        let command = &buffer[2..4];
        match command {
            b"CK" => {
                let clock_sync_packet = ClockSyncPacket::read(&mut reader)?;
                Ok(ControlPacket::ClockSync(clock_sync_packet))
            }
            b"OK" | b"IN" | b"NO" | b"BY" => {
                let body = SessionInitiationPacket::read(&mut reader, command)?;
                Ok(ControlPacket::SessionInitiation(body))
            }
            _ => Err(Error::new(
                ErrorKind::InvalidData,
                format!("Unknown control packet, {}", String::from_utf8_lossy(command)),
            ))?,
        }
    }

    pub fn write_header<W: Write>(writer: &mut W, command: &[u8; 2]) -> std::io::Result<usize> {
        writer.write_all(&[255, 255])?;
        writer.write_all(command)?;
        Ok(4)
    }

    pub fn is_control_packet(buffer: &[u8]) -> bool {
        buffer.starts_with(&CONTROL_PACKET_HEADER)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_invalid_control_packet() {
        let data = vec![0, 0, 0, 0];
        let result = ControlPacket::from_be_bytes(&data);
        assert!(result.is_err());
        if let Err(e) = result {
            assert_eq!(e.kind(), ErrorKind::InvalidData);
        }
    }

    #[test]
    fn test_parse_too_short_control_packet() {
        let data = vec![255, 255, 67];
        let result = ControlPacket::from_be_bytes(&data);
        assert!(result.is_err());
        if let Err(e) = result {
            assert_eq!(e.kind(), ErrorKind::InvalidData);
        }
    }

    #[test]
    fn test_write_header() {
        let mut buffer = Vec::new();
        let command = b"CK";
        let result = ControlPacket::write_header(&mut buffer, command);
        assert!(result.is_ok());
        assert_eq!(buffer, vec![255, 255, 67, 75]);
    }

    #[test]
    fn test_is_control_packet() {
        let valid_packet = vec![255, 255, 67, 75];
        let invalid_packet = vec![0, 0, 0, 0];
        assert!(ControlPacket::is_control_packet(&valid_packet));
        assert!(!ControlPacket::is_control_packet(&invalid_packet));
    }

    #[test]
    fn test_parse_unknown_control_packet() {
        let data = vec![255, 255, 0, 0];
        let result = ControlPacket::from_be_bytes(&data);
        assert!(result.is_err());
        if let Err(e) = result {
            assert_eq!(e.kind(), ErrorKind::InvalidData);
            assert_eq!(e.to_string(), "Unknown control packet, \u{0}\u{0}");
        }
    }

    #[test]
    fn test_read_clock_sync_packet_2() {
        let buffer = [
            0xFF, 0xFF, 0x43, 0x4B, //header
            0xF5, 0x19, 0xAE, 0xB9, //sender ssrc
            0x02, //count
            0x00, 0x00, 0x00, //reserved
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, // timestamp 1
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, // timestamp 2
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03, // timestamp 3
        ]; // Example buffer for a ClockSync packet

        let result = ControlPacket::from_be_bytes(&buffer);
        assert!(result.is_ok());
        if let ControlPacket::ClockSync(packet) = result.unwrap() {
            assert_eq!(packet.count, 2);
            assert_eq!(packet.sender_ssrc, 4112101049);
            assert_eq!(packet.timestamps[0], 1);
            assert_eq!(packet.timestamps[1], 2);
            assert_eq!(packet.timestamps[2], 3);
        } else {
            panic!("Expected ClockSync packet");
        }
    }

    #[test]
    fn test_read_session_initiation_packet() {
        let buffer = [
            0xFF, 0xFF, 0x49, 0x4E, //header
            0x00, 0x00, 0x00, 0x02, //version
            0xF8, 0xD1, 0x80, 0xE6, //initiator token
            0xF5, 0x19, 0xAE, 0xB9, //sender ssrc
            0x4C, 0x6F, 0x76, 0x65, 0x6C, 0x79, 0x20, 0x53, 0x65, 0x73, 0x73, 0x69, 0x6F, 0x6E, 0x00, //name
        ];

        let result = ControlPacket::from_be_bytes(&buffer);
        assert!(result.is_ok());
        if let ControlPacket::SessionInitiation(_packet) = result.unwrap() {
            // all good!
        } else {
            panic!("Expected SessionInitiation packet");
        }
    }
}

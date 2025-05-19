use std::io::{Cursor, Error, ErrorKind, Write};

use super::{
    clock_sync_packet::ClockSyncPacket,
    session_initiation_packet::SessionInitiationPacket, // Ensure ReadOptionalStringExt is in scope if SessionInitiationPacket::read relies on it being used on the reader directly.
};

#[derive(Debug)]
pub enum ControlPacket {
    ClockSync(ClockSyncPacket),
    SessionInitiation(SessionInitiationPacket),
}

impl ControlPacket {
    pub(crate) const HEADER_SIZE: usize = 4;

    pub fn from_be_bytes(buffer: &[u8]) -> std::io::Result<ControlPacket> {
        if buffer.len() < 4 {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "Buffer too short to be a valid control packet",
            ));
        }

        let command = if buffer[0] == 255 && buffer[1] == 255 {
            &buffer[2..4]
        } else {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "Invalid control packet header",
            ));
        };
        let mut reader = Cursor::new(&buffer[4..]);
        match command {
            b"CK" => {
                let clock_sync_packet = ClockSyncPacket::read(&mut reader)?;
                return Ok(ControlPacket::ClockSync(clock_sync_packet));
            }
            b"OK" | b"IN" | b"NO" | b"BY" => {
                let body = SessionInitiationPacket::read(&mut reader, command)?;
                return Ok(ControlPacket::SessionInitiation(body));
            }
            _ => Err(Error::new(
                ErrorKind::InvalidData,
                format!(
                    "Unknown control packet, {}",
                    String::from_utf8_lossy(command)
                ),
            ))?,
        }
    }

    pub fn write_header<W: Write>(writer: &mut W, command: &[u8; 2]) -> std::io::Result<usize> {
        writer.write_all(&[255, 255])?;
        writer.write_all(command)?;
        Ok(4)
    }

    pub fn is_control_packet(buffer: &[u8]) -> bool {
        buffer.len() >= 4 && buffer[0] == 255 && buffer[1] == 255
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
        let result = ControlPacket::write_header(&mut buffer, &command);
        assert!(result.is_ok());
        assert_eq!(buffer, vec![255, 255, 67, 75]);
    }
}

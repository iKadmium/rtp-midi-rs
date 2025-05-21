use crate::util::ReadOptionalStringExt;
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::io::{Read, Write};

use super::control_packet::ControlPacket;

#[derive(Debug)]
pub enum SessionInitiationPacket {
    Invitation(SessionInitiationPacketBody),
    Acknowledgment(SessionInitiationPacketBody),
    Rejection(SessionInitiationPacketBody),
    Termination(SessionInitiationPacketBody),
}

#[derive(Debug)]
pub struct SessionInitiationPacketBody {
    pub protocol_version: u32,
    pub initiator_token: u32,
    pub sender_ssrc: u32,
    pub name: Option<String>,
}

impl SessionInitiationPacket {
    pub fn new_acknowledgment(initiator_token: u32, sender_ssrc: u32, name: String) -> Self {
        SessionInitiationPacket::Acknowledgment(SessionInitiationPacketBody {
            protocol_version: 2,
            initiator_token,
            sender_ssrc,
            name: Some(name),
        })
    }

    pub fn new_invitation(initiator_token: u32, sender_ssrc: u32, name: String) -> Self {
        SessionInitiationPacket::Invitation(SessionInitiationPacketBody {
            protocol_version: 2,
            initiator_token,
            sender_ssrc,
            name: Some(name),
        })
    }

    pub fn new_rejection(initiator_token: u32, sender_ssrc: u32, name: String) -> Self {
        SessionInitiationPacket::Rejection(SessionInitiationPacketBody {
            protocol_version: 2,
            initiator_token,
            sender_ssrc,
            name: Some(name),
        })
    }

    pub fn new_termination(initiator_token: u32, sender_ssrc: u32) -> Self {
        SessionInitiationPacket::Termination(SessionInitiationPacketBody {
            protocol_version: 2,
            initiator_token,
            sender_ssrc,
            name: None,
        })
    }

    pub fn read<R: Read>(reader: &mut R, command: &[u8]) -> std::io::Result<Self> {
        let body = SessionInitiationPacketBody::read(reader)?;

        match command {
            b"IN" => Ok(SessionInitiationPacket::Invitation(body)),
            b"OK" => Ok(SessionInitiationPacket::Acknowledgment(body)),
            b"NO" => Ok(SessionInitiationPacket::Rejection(body)),
            b"BY" => Ok(SessionInitiationPacket::Termination(body)),
            _ => Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Unknown session initiation command")),
        }
    }

    fn body(&self) -> &SessionInitiationPacketBody {
        match self {
            SessionInitiationPacket::Invitation(body)
            | SessionInitiationPacket::Acknowledgment(body)
            | SessionInitiationPacket::Rejection(body)
            | SessionInitiationPacket::Termination(body) => body,
        }
    }

    fn command(&self) -> &[u8; 2] {
        match self {
            SessionInitiationPacket::Invitation(_) => b"IN",
            SessionInitiationPacket::Acknowledgment(_) => b"OK",
            SessionInitiationPacket::Rejection(_) => b"NO",
            SessionInitiationPacket::Termination(_) => b"BY",
        }
    }

    pub fn initiator_token(&self) -> u32 {
        self.body().initiator_token
    }

    pub(crate) fn write<W: Write>(&self, writer: &mut W) -> std::io::Result<usize> {
        let command = self.command();

        let mut length = ControlPacket::write_header(writer, command)?;
        length += self.body().write(writer)?;
        Ok(length)
    }

    pub(crate) fn to_bytes(&self) -> Vec<u8> {
        let mut buffer = Vec::with_capacity(self.size());
        self.write(&mut buffer).expect("Failed to write SessionInitiationPacket");
        buffer
    }

    pub(crate) fn size(&self) -> usize {
        ControlPacket::HEADER_SIZE + self.body().size()
    }

    pub fn ssrc(&self) -> u32 {
        self.body().sender_ssrc
    }

    pub fn protocol_version(&self) -> u32 {
        self.body().protocol_version
    }
}

impl SessionInitiationPacketBody {
    pub const MIN_SIZE: usize = size_of::<u32>() * 3;

    pub fn read<R: Read>(reader: &mut R) -> std::io::Result<Self> {
        let protocol_version = reader.read_u32::<BigEndian>()?;
        let initiator_token = reader.read_u32::<BigEndian>()?;
        let sender_ssrc = reader.read_u32::<BigEndian>()?;
        let name = reader.read_optional_string()?;

        Ok(SessionInitiationPacketBody {
            protocol_version,
            initiator_token,
            sender_ssrc,
            name,
        })
    }

    pub fn write<W: Write>(&self, writer: &mut W) -> std::io::Result<usize> {
        let length = self.size();

        writer.write_u32::<BigEndian>(self.protocol_version)?;
        writer.write_u32::<BigEndian>(self.initiator_token)?;
        writer.write_u32::<BigEndian>(self.sender_ssrc)?;

        if let Some(name) = &self.name {
            writer.write_all(name.as_bytes())?;
            writer.write_u8(0)?;
        }

        Ok(length)
    }

    pub fn size(&self) -> usize {
        SessionInitiationPacketBody::MIN_SIZE + self.name.as_ref().map_or(0, |name| name.len() + 1)
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use crate::packets::control_packets::session_initiation_packet::{SessionInitiationPacket, SessionInitiationPacketBody};

    fn get_test_body() -> [u8; 12] {
        [
            0x00, 0x00, 0x00, 0x02, //version
            0xF8, 0xD1, 0x80, 0xE6, //initiator token
            0xF5, 0x19, 0xAE, 0xB9, //sender ssrc
        ]
    }

    fn test_session_initiation_read_type(command: &[u8; 2]) {
        let mut cursor = Cursor::new(get_test_body());
        let result = SessionInitiationPacket::read(&mut cursor, command);
        match result {
            Ok(packet) => {
                assert_eq!(packet.command(), command);
                assert_eq!(packet.body().protocol_version, 2);
                assert_eq!(packet.body().initiator_token, 0xF8D180E6);
                assert_eq!(packet.body().sender_ssrc, 0xF519AEB9);
                assert_eq!(packet.body().name, None);
            }
            Err(e) => panic!("Failed to read packet: {}", e),
        }
    }

    #[test]
    fn test_read_invitation() {
        test_session_initiation_read_type(b"IN");
    }

    #[test]
    fn test_read_acknowledgement() {
        test_session_initiation_read_type(b"OK");
    }

    #[test]
    fn test_read_rejection() {
        test_session_initiation_read_type(b"NO");
    }

    #[test]
    fn test_read_termination() {
        test_session_initiation_read_type(b"BY");
    }

    #[test]
    fn test_read_body() {
        let buffer = [
            0x00, 0x00, 0x00, 0x02, //version
            0xF8, 0xD1, 0x80, 0xE6, //initiator token
            0xF5, 0x19, 0xAE, 0xB9, //sender ssrc
            0x4C, 0x6F, 0x76, 0x65, 0x6C, 0x79, 0x20, 0x53, 0x65, 0x73, 0x73, 0x69, 0x6F, 0x6E, 0x00, //name
        ];

        let mut cursor = Cursor::new(buffer);
        let result = SessionInitiationPacketBody::read(&mut cursor);
        match result {
            Ok(body) => {
                assert_eq!(body.protocol_version, 2);
                assert_eq!(body.initiator_token, 0xF8D180E6);
                assert_eq!(body.sender_ssrc, 0xF519AEB9);
                assert_eq!(body.name, Some("Lovely Session".to_string()));
            }
            Err(e) => panic!("Failed to read body: {}", e),
        }
    }

    #[test]
    fn test_read_invalid() {
        let mut cursor = Cursor::new(get_test_body());
        let result = SessionInitiationPacket::read(&mut cursor, b"XY");
        assert!(result.is_err());
    }

    #[test]
    fn test_new_acknowledgment() {
        let initiator_token = 0xF8D180E6;
        let sender_ssrc = 0xF519AEB9;
        let name = "Lovely Session".to_string();
        let packet = SessionInitiationPacket::new_acknowledgment(initiator_token, sender_ssrc, name.clone());
        if let SessionInitiationPacket::Acknowledgment(body) = packet {
            assert_eq!(body.protocol_version, 2);
            assert_eq!(body.initiator_token, initiator_token);
            assert_eq!(body.sender_ssrc, sender_ssrc);
            assert_eq!(body.name, Some(name));
        } else {
            panic!("Expected Acknowledgment packet");
        }
    }

    #[test]
    fn test_new_invitation() {
        let initiator_token = 0xF8D180E6;
        let sender_ssrc = 0xF519AEB9;
        let name = "Lovely Session".to_string();
        let packet = SessionInitiationPacket::new_invitation(initiator_token, sender_ssrc, name.clone());
        if let SessionInitiationPacket::Invitation(body) = packet {
            assert_eq!(body.protocol_version, 2);
            assert_eq!(body.initiator_token, initiator_token);
            assert_eq!(body.sender_ssrc, sender_ssrc);
            assert_eq!(body.name, Some(name));
        } else {
            panic!("Expected Invitation packet");
        }
    }

    #[test]
    fn test_write() {
        let initiator_token = 0xF8D180E6;
        let sender_ssrc = 0xF519AEB9;
        let name = "Lovely Session".to_string();
        let packet = SessionInitiationPacket::new_acknowledgment(initiator_token, sender_ssrc, name.clone());
        let mut buffer = Vec::new();
        let result = packet.write(&mut buffer);
        assert!(result.is_ok());
        let length = result.unwrap();
        assert_eq!(length, packet.size());
        assert_eq!(buffer.len(), length);
        assert_eq!(&buffer[0..2], &[255, 255]);
        assert_eq!(&buffer[2..4], b"OK");
        assert_eq!(&buffer[4..8], &get_test_body()[0..4]);
        assert_eq!(&buffer[8..12], &get_test_body()[4..8]);
        assert_eq!(&buffer[12..16], &get_test_body()[8..12]);
        assert_eq!(&buffer[16..30], name.as_bytes());
        assert_eq!(buffer[30], 0); // Null terminator for the name
    }
}

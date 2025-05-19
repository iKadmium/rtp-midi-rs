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

    pub fn read<R: Read>(reader: &mut R, command: &[u8]) -> std::io::Result<Self> {
        let body = SessionInitiationPacketBody::read(reader)?;

        match command {
            b"IN" => Ok(SessionInitiationPacket::Invitation(body)),
            b"OK" => Ok(SessionInitiationPacket::Acknowledgment(body)),
            b"NO" => Ok(SessionInitiationPacket::Rejection(body)),
            b"BY" => Ok(SessionInitiationPacket::Termination(body)),
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Unknown session initiation command",
            )),
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

    pub(crate) fn write<W: Write>(&self, writer: &mut W) -> std::io::Result<usize> {
        let command = match self {
            SessionInitiationPacket::Invitation(_) => b"IN",
            SessionInitiationPacket::Acknowledgment(_) => b"OK",
            SessionInitiationPacket::Rejection(_) => b"NO",
            SessionInitiationPacket::Termination(_) => b"BY",
        };

        let mut length = ControlPacket::write_header(writer, command)?;
        length += self.body().write(writer)?;
        Ok(length)
    }

    pub fn size(&self) -> usize {
        ControlPacket::HEADER_SIZE + self.body().size()
    }
}

impl SessionInitiationPacketBody {
    pub const MIN_SIZE: usize = size_of::<u32>() * 12;

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
    use crate::packet::control_packets::{
        control_packet::ControlPacket, session_initiation_packet::SessionInitiationPacket,
    };

    #[test]
    fn test_read_invitation() {
        let buffer = [
            0xFF, 0xFF, b'I', b'N', //header
            0x00, 0x00, 0x00, 0x02, //version
            0xF8, 0xD1, 0x80, 0xE6, //initiator token
            0xF5, 0x19, 0xAE, 0xB9, //sender ssrc
            0x4C, 0x6F, 0x76, 0x65, 0x6C, 0x79, 0x20, 0x53, 0x65, 0x73, 0x73, 0x69, 0x6F, 0x6E,
            0x00, //name
        ];

        let result = ControlPacket::from_be_bytes(&buffer);
        assert!(result.is_ok());
        if let ControlPacket::SessionInitiation(packet) = result.unwrap() {
            match packet {
                SessionInitiationPacket::Invitation(invitation) => {
                    assert_eq!(invitation.protocol_version, 2);
                    assert_eq!(invitation.initiator_token, 0xF8D180E6);
                    assert_eq!(invitation.sender_ssrc, 0xF519AEB9);
                    assert_eq!(invitation.name, Some("Lovely Session".to_string()));
                }
                _ => panic!("Expected Acknowledgment packet"),
            }
        } else {
            panic!("Expected SessionInitiation packet");
        }
    }

    #[test]
    fn test_read_acknowledgement() {
        let buffer = [
            0xFF, 0xFF, b'O', b'K', //header
            0x00, 0x00, 0x00, 0x02, //version
            0xF8, 0xD1, 0x80, 0xE6, //initiator token
            0xF5, 0x19, 0xAE, 0xB9, //sender ssrc
        ];

        let result = ControlPacket::from_be_bytes(&buffer);
        assert!(result.is_ok());
        if let ControlPacket::SessionInitiation(packet) = result.unwrap() {
            match packet {
                SessionInitiationPacket::Acknowledgment(invitation) => {
                    assert_eq!(invitation.protocol_version, 2);
                    assert_eq!(invitation.initiator_token, 0xF8D180E6);
                    assert_eq!(invitation.sender_ssrc, 0xF519AEB9);
                    assert_eq!(invitation.name, None);
                }
                _ => panic!("Expected Acknowledgment packet"),
            }
        } else {
            panic!("Expected SessionInitiation packet");
        }
    }

    #[test]
    fn test_read_rejection() {
        let buffer = [
            0xFF, 0xFF, b'N', b'O', //header
            0x00, 0x00, 0x00, 0x02, //version
            0xF8, 0xD1, 0x80, 0xE6, //initiator token
            0xF5, 0x19, 0xAE, 0xB9, //sender ssrc
        ];

        let result = ControlPacket::from_be_bytes(&buffer);
        assert!(result.is_ok());
        if let ControlPacket::SessionInitiation(packet) = result.unwrap() {
            match packet {
                SessionInitiationPacket::Rejection(invitation) => {
                    assert_eq!(invitation.protocol_version, 2);
                    assert_eq!(invitation.initiator_token, 0xF8D180E6);
                    assert_eq!(invitation.sender_ssrc, 0xF519AEB9);
                    assert_eq!(invitation.name, None);
                }
                _ => panic!("Expected Rejection packet"),
            }
        } else {
            panic!("Expected SessionInitiation packet");
        }
    }

    #[test]
    fn test_read_termination() {
        let buffer = [
            0xFF, 0xFF, b'B', b'Y', //header
            0x00, 0x00, 0x00, 0x02, //version
            0xF8, 0xD1, 0x80, 0xE6, //initiator token
            0xF5, 0x19, 0xAE, 0xB9, //sender ssrc
        ];

        let result = ControlPacket::from_be_bytes(&buffer);
        assert!(result.is_ok());
        if let ControlPacket::SessionInitiation(packet) = result.unwrap() {
            match packet {
                SessionInitiationPacket::Termination(invitation) => {
                    assert_eq!(invitation.protocol_version, 2);
                    assert_eq!(invitation.initiator_token, 0xF8D180E6);
                    assert_eq!(invitation.sender_ssrc, 0xF519AEB9);
                    assert_eq!(invitation.name, None);
                }
                _ => panic!("Expected Termination packet"),
            }
        } else {
            panic!("Expected SessionInitiation packet");
        }
    }
}

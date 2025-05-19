use crate::util::ReadOptionalStringExt;
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::{
    fmt,
    io::{Read, Write},
};

use super::control_packet::ControlPacket;

#[derive(Debug)]
pub enum SessionInitiationPacket {
    Invitation(SessionInitiationPacketBody),
    Acknowledgment(SessionInitiationPacketBody),
    Rejection(SessionInitiationPacketBody),
    Termination(SessionInitiationPacketBody),
}

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

    pub fn write<W: Write>(&self, writer: &mut W) -> std::io::Result<usize> {
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
}

impl SessionInitiationPacketBody {
    pub const MIN_SIZE: usize = 16;

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

impl fmt::Debug for SessionInitiationPacketBody {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SessionInitiationPacket")
            .field("protocol_version", &self.protocol_version)
            .field("initiator_token", &self.initiator_token)
            .field("sender_ssrc", &self.sender_ssrc)
            .field("name", &self.name)
            .finish()
    }
}

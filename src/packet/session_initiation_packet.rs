use std::{
    fmt,
    io::{Error, ErrorKind},
};

pub struct SessionInitiationPacket {
    pub command: [u8; 2],
    pub protocol_version: u32,
    pub initiator_token: u32,
    pub sender_ssrc: u32,
    pub name: Option<String>,
}

impl SessionInitiationPacket {
    pub const MIN_SIZE: usize = 16;

    pub fn parse(buffer: &[u8]) -> Result<Self, Error> {
        if buffer.len() < SessionInitiationPacket::MIN_SIZE {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "Buffer too short to be a valid session initiation packet",
            ));
        }

        let command = buffer[2..4].try_into().unwrap();
        let protocol_version = u32::from_be_bytes(buffer[4..8].try_into().unwrap());
        let initiator_token = u32::from_be_bytes(buffer[8..12].try_into().unwrap());
        let sender_ssrc = u32::from_be_bytes(buffer[12..16].try_into().unwrap());

        let name = if buffer.len() > 16 {
            let name_bytes = &buffer[16..];
            match name_bytes.iter().position(|&b| b == 0) {
                Some(pos) => Some(String::from_utf8_lossy(&name_bytes[..pos]).to_string()),
                None => None,
            }
        } else {
            None
        };

        Ok(SessionInitiationPacket {
            command,
            protocol_version,
            initiator_token,
            sender_ssrc,
            name,
        })
    }

    pub fn write_to_bytes(&self, bytes: &mut [u8]) -> std::io::Result<usize> {
        let length = self.size();

        if bytes.len() < length {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "Buffer too short to write session initiation packet",
            ));
        }

        bytes[0] = 255;
        bytes[1] = 255;

        bytes[2..4].copy_from_slice(&self.command);
        bytes[4..8].copy_from_slice(&self.protocol_version.to_be_bytes());
        bytes[8..12].copy_from_slice(&self.initiator_token.to_be_bytes());
        bytes[12..16].copy_from_slice(&self.sender_ssrc.to_be_bytes());

        if let Some(name) = &self.name {
            bytes[16..16 + name.len()].copy_from_slice(name.as_bytes());
            bytes[16 + name.len()] = 0; // Null terminator
        }

        Ok(length)
    }

    pub fn size(&self) -> usize {
        SessionInitiationPacket::MIN_SIZE + self.name.as_ref().map_or(0, |name| name.len() + 1)
    }
}

impl fmt::Debug for SessionInitiationPacket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SessionInitiationPacket")
            .field("command", &String::from_utf8_lossy(&self.command))
            .field("protocol_version", &self.protocol_version)
            .field("initiator_token", &self.initiator_token)
            .field("sender_ssrc", &self.sender_ssrc)
            .field("name", &self.name)
            .finish()
    }
}

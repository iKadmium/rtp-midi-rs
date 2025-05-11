use std::fmt;

pub struct SessionInitiationPacket {
    pub command: [u8; 2],
    pub protocol_version: u32,
    pub initiator_token: u32,
    pub sender_ssrc: u32,
    pub name: Option<String>,
}

impl SessionInitiationPacket {
    pub fn parse(buffer: &[u8]) -> Result<Self, String> {
        if buffer.len() < 16 {
            return Err("Buffer too short to be a valid session initiation packet".to_string());
        }

        if !Self::has_valid_header(buffer) {
            return Err("Invalid header: does not start with 0xFFFF".to_string());
        }

        let command = [buffer[2], buffer[3]];
        let protocol_version = u32::from_be_bytes([buffer[4], buffer[5], buffer[6], buffer[7]]);
        let initiator_token = u32::from_be_bytes([buffer[8], buffer[9], buffer[10], buffer[11]]);
        let sender_ssrc = u32::from_be_bytes([buffer[12], buffer[13], buffer[14], buffer[15]]);

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

    fn has_valid_header(buffer: &[u8]) -> bool {
        buffer.len() >= 4
            && buffer[0] == 255
            && buffer[1] == 255
            && buffer[2] == b'I'
            && buffer[3] == b'N'
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buffer = Vec::new();

        // Add the header
        buffer.push(255);
        buffer.push(255);

        // Add the command
        buffer.extend_from_slice(&self.command);

        // Add the protocol version
        buffer.extend_from_slice(&self.protocol_version.to_be_bytes());

        // Add the initiator token
        buffer.extend_from_slice(&self.initiator_token.to_be_bytes());

        // Add the sender SSRC
        buffer.extend_from_slice(&self.sender_ssrc.to_be_bytes());

        // Add the name if it exists
        if let Some(name) = &self.name {
            buffer.extend_from_slice(name.as_bytes());
            buffer.push(0); // Null terminator
        }

        buffer
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

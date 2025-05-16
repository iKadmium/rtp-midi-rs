use std::io::{Error, ErrorKind};

use super::{
    clock_sync_packet::ClockSyncPacket, session_initiation_packet::SessionInitiationPacket,
};

#[derive(Debug)]
pub enum ControlPacket {
    ClockSync(ClockSyncPacket),
    SessionInitiation(SessionInitiationPacket),
    EndSession,
}

impl ControlPacket {
    pub fn parse(buffer: &[u8]) -> Result<Self, std::io::Error> {
        let command = if buffer.len() > 2 && buffer[0] == 255 && buffer[1] == 255 {
            String::from_utf8_lossy(&buffer[2..4]).to_string()
        } else {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "Invalid control packet header",
            ));
        };
        match command.as_str() {
            "CK" => {
                let clock_sync_packet = ClockSyncPacket::from_be_bytes(buffer)?;
                return Ok(ControlPacket::ClockSync(clock_sync_packet));
            }
            "IN" => {
                let session_initiation_packet = SessionInitiationPacket::parse(buffer)?;
                return Ok(ControlPacket::SessionInitiation(session_initiation_packet));
            }
            "BY" => {
                return Ok(ControlPacket::EndSession);
            }
            _ => Err(Error::new(
                ErrorKind::InvalidData,
                format!("Unknown control packet, {}", command),
            ))?,
        }
    }

    pub fn is_control_packet(buffer: &[u8]) -> bool {
        buffer.len() >= 2 && buffer[0] == 255 && buffer[1] == 255
    }
}

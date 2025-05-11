#[derive(Debug)]
pub struct ControlPacket {
    pub command: Option<String>,
}

impl ControlPacket {
    pub fn parse_header(buffer: &[u8]) -> Option<Self> {
        if Self::has_valid_header(buffer) {
            let command_type = if buffer.len() > 2 && buffer[0] == 255 && buffer[1] == 255 {
                Some(String::from_utf8_lossy(&buffer[2..4]).to_string())
            } else {
                None
            };
            Some(ControlPacket {
                command: command_type,
            })
        } else {
            None
        }
    }

    fn has_valid_header(buffer: &[u8]) -> bool {
        buffer.len() >= 2 && buffer[0] == 255 && buffer[1] == 255
    }
}

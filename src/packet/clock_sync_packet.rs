use std::fmt;

pub struct ClockSyncPacket {
    pub count: u8,
    pub timestamps: Vec<u64>,
    pub sender_ssrc: u32,
}

impl ClockSyncPacket {
    pub fn parse(buffer: &[u8]) -> Result<Self, String> {
        if buffer.len() < 12 {
            return Err("Buffer too short to be a valid clock sync packet".to_string());
        }

        if !Self::has_valid_header(buffer) {
            return Err("Invalid header: does not start with 0xFFFF".to_string());
        }

        let sender_ssrc = u32::from_be_bytes(buffer[4..8].try_into().unwrap());
        let count = buffer[8];

        let mut timestamps = Vec::new();

        for i in (8..buffer.len()).step_by(4) {
            timestamps.push(u64::from_be_bytes([
                0,
                0,
                0,
                0,
                buffer[i],
                buffer[i + 1],
                buffer[i + 2],
                buffer[i + 3],
            ]));
        }

        Ok(ClockSyncPacket {
            count,
            timestamps,
            sender_ssrc,
        })
    }

    fn has_valid_header(buffer: &[u8]) -> bool {
        buffer.len() >= 4
            && buffer[0] == 255
            && buffer[1] == 255
            && buffer[2] == b'C'
            && buffer[3] == b'K'
    }

    pub(crate) fn to_bytes(&self) -> Vec<u8> {
        let mut buffer = Vec::new();

        // Add the header
        buffer.push(255);
        buffer.push(255);
        buffer.push(b'C');
        buffer.push(b'K');

        // Add the sender SSRC
        buffer.extend_from_slice(&self.sender_ssrc.to_be_bytes());

        // Add the count
        buffer.push(self.count);

        // Add the timestamps
        for timestamp in &self.timestamps {
            buffer.extend_from_slice(&timestamp.to_be_bytes()[4..]);
        }

        buffer
    }
}

impl fmt::Debug for ClockSyncPacket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClockSyncPacket")
            .field("count", &self.count)
            .field("timestamps", &self.timestamps)
            .field("sender_ssrc", &self.sender_ssrc)
            .finish()
    }
}

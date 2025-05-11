use std::fmt;

pub struct ClockSyncPacket {
    pub count: u8,
    pub timestamps: [Option<u64>; 3],
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

        let sender_ssrc = u32::from_be_bytes([buffer[4], buffer[5], buffer[6], buffer[7]]);
        let count = buffer[8];

        let timestamps = [
            if buffer.len() >= 16 {
                Some(u64::from_be_bytes([
                    0, 0, 0, 0, buffer[12], buffer[13], buffer[14], buffer[15],
                ]))
            } else {
                None
            },
            if buffer.len() >= 20 {
                Some(u64::from_be_bytes([
                    0, 0, 0, 0, buffer[16], buffer[17], buffer[18], buffer[19],
                ]))
            } else {
                None
            },
            if buffer.len() >= 24 {
                Some(u64::from_be_bytes([
                    0, 0, 0, 0, buffer[20], buffer[21], buffer[22], buffer[23],
                ]))
            } else {
                None
            },
        ];

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

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buffer = Vec::new();

        // Add the header
        buffer.push(0xFF);
        buffer.push(0xFF);
        buffer.push(b'C');
        buffer.push(b'K');

        // Add the sender SSRC
        buffer.extend_from_slice(&self.sender_ssrc.to_be_bytes());

        // Add the count
        buffer.push(self.count);

        // Add unused bytes (3 bytes set to 0)
        buffer.extend_from_slice(&[0, 0, 0]);

        // Add the timestamps
        for &timestamp in &self.timestamps {
            if let Some(ts) = timestamp {
                buffer.extend_from_slice(&ts.to_be_bytes());
            }
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

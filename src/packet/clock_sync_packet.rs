use std::{
    fmt,
    io::{Error, ErrorKind},
};

pub struct ClockSyncPacket {
    pub count: u8,
    pub timestamps: [u64; 3],
    pub sender_ssrc: u32,
}

impl ClockSyncPacket {
    pub const SIZE: usize = 36;

    pub fn new(count: u8, timestamps: [u64; 3], sender_ssrc: u32) -> Self {
        ClockSyncPacket {
            count,
            timestamps,
            sender_ssrc,
        }
    }

    pub fn from_be_bytes(buffer: &[u8]) -> Result<Self, Error> {
        if buffer.len() < 12 {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "Buffer too short to be a valid clock sync packet",
            ));
        }

        if !Self::has_valid_header(buffer) {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "Header is not valid for a clock sync packet",
            ));
        }

        let sender_ssrc = u32::from_be_bytes(buffer[4..8].try_into().unwrap());
        let count = buffer[8];

        let mut timestamps = [0; 3];

        for i in 0..3 {
            timestamps[i] = u64::from_be_bytes(buffer[12 + i * 8..20 + i * 8].try_into().unwrap());
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

    pub fn write_to_bytes(&self, bytes: &mut [u8]) -> std::io::Result<usize> {
        bytes[0] = 255;
        bytes[1] = 255;
        bytes[2] = b'C';
        bytes[3] = b'K';

        bytes[4..8].copy_from_slice(&self.sender_ssrc.to_be_bytes());

        bytes[8] = self.count;
        bytes[9..12].copy_from_slice(&[0, 0, 0]); // Reserved bytes

        bytes[12..20].copy_from_slice(&self.timestamps[0].to_be_bytes());
        bytes[20..28].copy_from_slice(&self.timestamps[1].to_be_bytes());
        bytes[28..36].copy_from_slice(&self.timestamps[2].to_be_bytes());

        Ok(size_of::<Self>())
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

#[allow(dead_code)]
pub struct MidiPacketHeader {
    flags: u16, // 2 bits for version, 1 bit for p_flag, 1 bit for x_flag, 4 bits for cc, 1 bit for m_flag, 7 bits for pt
    sequence_number: u16, // Sequence number
    timestamp: u32, // Lower 32 bits of the timestamp in 100-microsecond units
    ssrc: u32,  // Sender SSRC
}

impl MidiPacketHeader {
    pub fn new(sequence_number: u16, timestamp: u32, ssrc: u32) -> Self {
        //let flags: u8 = 0b10

        MidiPacketHeader {
            flags: 0,
            sequence_number,
            timestamp,
            ssrc,
        }
    }

    pub fn version(&self) -> u8 {
        ((self.flags & 0b1100_0000_0000_0000) >> 6) as u8
    }

    pub fn p_flag(&self) -> u8 {
        ((self.flags & 0b0010_0000_0000_0000) >> 5) as u8
    }

    pub fn x_flag(&self) -> u8 {
        ((self.flags & 0b0001_0000_0000_0000) >> 4) as u8
    }

    pub fn cc(&self) -> u8 {
        ((self.flags & 0b0000_1111_0000_0000) >> 8) as u8
    }

    pub fn m_flag(&self) -> u8 {
        ((self.flags & 0b0000_0000_1000_0000) >> 7) as u8
    }

    pub fn pt(&self) -> u8 {
        (self.flags & 0b0000_0000_0111_1111) as u8
    }

    pub fn sequence_number(&self) -> u16 {
        self.sequence_number
    }

    pub fn timestamp(&self) -> u32 {
        self.timestamp
    }

    pub fn ssrc(&self) -> u32 {
        self.ssrc
    }

    pub fn from_be_bytes(bytes: &[u8]) -> Result<Self, std::io::Error> {
        if bytes.len() < 12 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid header length",
            ));
        }

        Ok(Self {
            flags: u16::from_be_bytes(bytes[0..2].try_into().unwrap()),
            sequence_number: u16::from_be_bytes(bytes[2..4].try_into().unwrap()),
            timestamp: u32::from_be_bytes(bytes[4..8].try_into().unwrap()),
            ssrc: u32::from_be_bytes(bytes[8..12].try_into().unwrap()),
        })
    }

    pub fn write_to_bytes(&self, bytes: &mut [u8]) -> Result<usize, std::io::Error> {
        if bytes.len() < 12 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid header length",
            ));
        }

        bytes[0..2].copy_from_slice(&self.flags.to_be_bytes());
        bytes[2..4].copy_from_slice(&self.sequence_number.to_be_bytes());
        bytes[4..8].copy_from_slice(&self.timestamp.to_be_bytes());
        bytes[8..12].copy_from_slice(&self.ssrc.to_be_bytes());

        Ok(12)
    }
}

impl std::fmt::Debug for MidiPacketHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MidiPacketHeader")
            .field(
                "flags",
                &format_args!(
                    "version: {}, p: {}, x: {}, cc: {}, m: {}, pt: {}",
                    self.version(),
                    self.p_flag(),
                    self.x_flag(),
                    self.cc(),
                    self.m_flag(),
                    self.pt()
                ),
            )
            .field("sequence_number", &self.sequence_number)
            .field("timestamp", &self.timestamp)
            .field("ssrc", &self.ssrc)
            .finish()
    }
}

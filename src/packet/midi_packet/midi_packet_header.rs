#[allow(dead_code)]
pub struct MidiPacketHeader {
    flags: MidiPacketHeaderFlags, // 2 bits for version, 1 bit for p_flag, 1 bit for x_flag, 4 bits for cc, 1 bit for m_flag, 7 bits for pt
    sequence_number: u16,         // Sequence number
    timestamp: u32,               // Lower 32 bits of the timestamp in 100-microsecond units
    ssrc: u32,                    // Sender SSRC
}

#[repr(u16)]
pub enum FlagMasks {
    Version = 0b1100_0000_0000_0000,
    P = 0b0010_0000_0000_0000,
    X = 0b0001_0000_0000_0000,
    CC = 0b0000_1111_0000_0000,
    M = 0b0000_0000_1000_0000,
    PT = 0b0000_0000_0111_1111,
}

pub struct MidiPacketHeaderFlags {
    flags: u16,
}

impl MidiPacketHeaderFlags {
    pub fn new(version: u8, p: bool, x: bool, cc: u8, m: bool, pt: u8) -> Self {
        let mut flags = MidiPacketHeaderFlags { flags: 0 };
        flags.set_version(version);
        flags.set_flag(FlagMasks::P, p);
        flags.set_flag(FlagMasks::X, x);
        flags.set_cc(cc);
        flags.set_flag(FlagMasks::M, m);
        flags.set_pt(pt);
        flags
    }

    pub fn from_u16(flags: u16) -> Self {
        MidiPacketHeaderFlags { flags }
    }

    fn get_flag(&self, flag: FlagMasks) -> bool {
        self.flags & flag as u16 != 0
    }

    fn set_flag(&mut self, flag: FlagMasks, value: bool) {
        if value {
            self.flags |= flag as u16;
        } else {
            self.flags &= !(flag as u16);
        }
    }

    fn get_version(&self) -> u8 {
        ((self.flags & FlagMasks::Version as u16) >> 14) as u8
    }

    fn set_version(&mut self, version: u8) {
        self.flags = (self.flags & !(FlagMasks::Version as u16)) | ((version as u16) << 14);
    }

    fn cc(&self) -> u8 {
        ((self.flags & FlagMasks::CC as u16) >> 8) as u8
    }

    fn set_cc(&mut self, cc: u8) {
        self.flags = (self.flags & !(FlagMasks::CC as u16)) | ((cc as u16) << 8);
    }

    fn pt(&self) -> u8 {
        (self.flags & FlagMasks::PT as u16) as u8
    }

    fn set_pt(&mut self, pt: u8) {
        self.flags = (self.flags & !(FlagMasks::PT as u16)) | (pt as u16);
    }

    fn to_be_bytes(&self) -> [u8; 2] {
        self.flags.to_be_bytes()
    }
}

impl MidiPacketHeader {
    pub fn new(sequence_number: u16, timestamp: u32, ssrc: u32) -> Self {
        //let flags: u8 = 0b10
        let flags = MidiPacketHeaderFlags::new(2, false, false, 0, false, 97);

        MidiPacketHeader {
            flags,
            sequence_number,
            timestamp,
            ssrc,
        }
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

    pub fn flags(&self) -> &MidiPacketHeaderFlags {
        &self.flags
    }

    pub fn from_be_bytes(bytes: &[u8]) -> Result<Self, std::io::Error> {
        if bytes.len() < 12 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid header length",
            ));
        }

        Ok(Self {
            flags: MidiPacketHeaderFlags::from_u16(u16::from_be_bytes(
                bytes[0..2].try_into().unwrap(),
            )),
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
                    self.flags.get_version(),
                    self.flags.get_flag(FlagMasks::P),
                    self.flags.get_flag(FlagMasks::X),
                    self.flags.cc(),
                    self.flags.get_flag(FlagMasks::M),
                    self.flags.pt()
                ),
            )
            .field("sequence_number", &self.sequence_number)
            .field("timestamp", &self.timestamp)
            .field("ssrc", &self.ssrc)
            .finish()
    }
}

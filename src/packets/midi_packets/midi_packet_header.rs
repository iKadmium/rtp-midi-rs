use byteorder::{BigEndian, WriteBytesExt};
use std::io::Write;

pub(super) struct MidiPacketHeader {
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
}

impl From<u16> for MidiPacketHeaderFlags {
    fn from(flags: u16) -> Self {
        MidiPacketHeaderFlags { flags }
    }
}

impl From<MidiPacketHeaderFlags> for u16 {
    fn from(flags: MidiPacketHeaderFlags) -> u16 {
        flags.flags
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

    pub fn write<W: Write>(&self, writer: &mut W) -> Result<usize, std::io::Error> {
        writer.write_u16::<BigEndian>(self.flags.flags)?;
        writer.write_u16::<BigEndian>(self.sequence_number)?;
        writer.write_u32::<BigEndian>(self.timestamp)?;
        writer.write_u32::<BigEndian>(self.ssrc)?;

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

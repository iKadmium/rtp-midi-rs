use super::midi_command_list_body::MidiCommandListBody;

pub struct MidiCommandListHeader {
    flags: MidiCommandListFlags,
    length: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MidiCommandListFlags {
    flags: u8,
}

#[repr(u8)]
enum MidiCommandSectionFlagMasks {
    B = 0b1000_0000,
    J = 0b0100_0000,
    Z = 0b0010_0000,
    P = 0b0001_0000,
}

impl MidiCommandListFlags {
    pub fn new(b_flag: bool, j_flag: bool, z_flag: bool, p_flag: bool) -> Self {
        let mut this = MidiCommandListFlags { flags: 0 };
        this.set_flag(MidiCommandSectionFlagMasks::B, b_flag);
        this.set_flag(MidiCommandSectionFlagMasks::J, j_flag);
        this.set_flag(MidiCommandSectionFlagMasks::Z, z_flag);
        this.set_flag(MidiCommandSectionFlagMasks::P, p_flag);
        this
    }

    fn get_flag(&self, flag: MidiCommandSectionFlagMasks) -> bool {
        self.flags & flag as u8 != 0
    }

    fn set_flag(&mut self, flag: MidiCommandSectionFlagMasks, value: bool) {
        if value {
            self.flags |= flag as u8;
        } else {
            self.flags &= !(flag as u8);
        }
    }

    pub fn j_flag(&self) -> bool {
        self.get_flag(MidiCommandSectionFlagMasks::J)
    }

    pub fn b_flag(&self) -> bool {
        self.get_flag(MidiCommandSectionFlagMasks::B)
    }

    pub fn z_flag(&self) -> bool {
        self.get_flag(MidiCommandSectionFlagMasks::Z)
    }

    pub fn p_flag(&self) -> bool {
        self.get_flag(MidiCommandSectionFlagMasks::P)
    }

    pub fn needs_b_flag(size: usize) -> bool {
        size > 0x0F
    }

    pub fn from_be_bytes(bytes: &[u8]) -> Self {
        MidiCommandListFlags {
            flags: bytes[0] & 0xF0,
        }
    }
}

impl MidiCommandListHeader {
    pub const MAX_HEADER_SIZE: usize = 2;

    pub fn new(flags: MidiCommandListFlags, length: usize) -> Self {
        MidiCommandListHeader { flags, length }
    }

    pub fn flags(&self) -> &MidiCommandListFlags {
        &self.flags
    }

    pub fn length(&self) -> usize {
        self.length
    }

    pub fn size(b_flag: bool) -> usize {
        if b_flag { 2 } else { 1 }
    }

    pub fn build_for(body: &MidiCommandListBody, j_flag: bool, z_flag: bool, p_flag: bool) -> Self {
        let length = body.size(z_flag);
        let flags = MidiCommandListFlags::new(
            MidiCommandListFlags::needs_b_flag(length),
            j_flag,
            z_flag,
            p_flag,
        );
        Self::new(flags, length)
    }

    pub fn write_to_bytes(&self, bytes: &mut [u8]) -> std::io::Result<usize> {
        if self.length > 0x0F {
            // If length > 0x0F, use 12 bits for length and 4 bits for flags
            // Set the high bit to indicate extended length
            let flags_and_length: u16 =
                0x8000 | ((self.flags.flags as u16) << 8) | ((self.length as u16) & 0x0FFF);
            bytes[0..2].copy_from_slice(&flags_and_length.to_be_bytes());
            Ok(2)
        } else {
            // Otherwise, use 4 bits for length and 4 bits for flags
            let flags_and_length: u8 = (self.flags.flags) | ((self.length as u8) & 0x000F);
            bytes[0] = flags_and_length;
            Ok(1)
        }
    }

    pub fn from_be_bytes(bytes: &[u8]) -> std::io::Result<Self> {
        let flags = MidiCommandListFlags::from_be_bytes(bytes);
        let result = if flags.b_flag() {
            let length_msb = bytes[0] & 0x0F;
            let length_lsb = bytes[1];
            let length = u16::from_be_bytes([length_msb, length_lsb]) as usize;
            Self { flags, length }
        } else {
            let length = (bytes[0] & 0x0F) as usize;
            Self { flags, length }
        };

        Ok(result)
    }
}

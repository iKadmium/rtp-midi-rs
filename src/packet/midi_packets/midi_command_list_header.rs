use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::io::{Read, Write};

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

    pub fn from_u8(byte: u8) -> Self {
        MidiCommandListFlags { flags: byte & 0xF0 }
    }
}

impl MidiCommandListHeader {
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

    pub fn write<W: Write>(&self, writer: &mut W) -> std::io::Result<usize> {
        if self.flags.b_flag() {
            // If b_flag is set, use 12 bits for length and 4 bits for flags
            // Set the high bit to indicate extended length
            let flags_and_length: u16 =
                0x8000 | ((self.flags.flags as u16) << 8) | ((self.length as u16) & 0x0FFF);
            writer.write_u16::<BigEndian>(flags_and_length)?;
            Ok(2)
        } else {
            // Otherwise, use 4 bits for length and 4 bits for flags
            let flags_and_length: u8 = (self.flags.flags) | ((self.length as u8) & 0x000F);
            writer.write_u8(flags_and_length)?;
            Ok(1)
        }
    }

    pub fn read<R: Read>(reader: &mut R) -> std::io::Result<Self> {
        let first_byte = reader.read_u8()?;
        let flags = MidiCommandListFlags::from_u8(first_byte);
        let result = if flags.b_flag() {
            let length_lsb = reader.read_u8()?;
            // Correctly reconstruct the 12-bit length from the two bytes
            // The first byte contains the 4 flag bits and the 4 MSBs of the length.
            // The second byte contains the 8 LSBs of the length.
            // So, length = ((first_byte & 0x0F) << 8) | length_lsb
            let length = (((first_byte & 0x0F) as u16) << 8) | (length_lsb as u16);
            Self {
                flags,
                length: length as usize,
            }
        } else {
            let length = (first_byte & 0x0F) as usize;
            Self { flags, length }
        };

        Ok(result)
    }
}

use log::trace;

use super::midi_timed_command::TimedCommand;

#[derive(Debug)]
#[allow(dead_code)]
pub struct MidiCommandSection {
    flags: u8,
    commands: Vec<TimedCommand>,
}

impl MidiCommandSection {
    pub fn new() -> Self {
        MidiCommandSection {
            flags: 0,
            commands: Vec::new(),
        }
    }

    pub fn b_flag(&self) -> bool {
        self.length() > 0x000F
    }

    pub fn j_flag(&self) -> bool {
        self.flags & 0b0100_0000 != 0
    }

    pub fn z_flag(&self) -> bool {
        self.flags & 0b0010_0000 != 0
    }

    pub fn p_flag(&self) -> bool {
        self.flags & 0b0001_0000 != 0
    }

    pub fn length(&self) -> usize {
        let mut length: usize = if self.b_flag() { 2 } else { 1 };
        let mut running_status: Option<u8> = None;
        for (i, command) in self.commands.iter().enumerate() {
            if i > 0 || self.z_flag() {
                match command.delta_time() {
                    Some(ref delta_time) => length += delta_time.size(),
                    None => {
                        length += 1;
                    }
                }
            }
            if Some(command.command().status) != running_status {
                length += 1;
            }
            length += command.command().data.len();
            running_status = Some(command.command().status);
        }
        return length;
    }

    pub fn commands(&self) -> &[TimedCommand] {
        &self.commands
    }

    pub fn from_be_bytes(bytes: &[u8]) -> Result<Self, std::io::Error> {
        trace!("Parsing MIDI command section from bytes, {:#?}", bytes);
        let flags_and_length_lo = bytes[0];
        let b_flag = flags_and_length_lo & 0b1000_0000 != 0;
        let flags = flags_and_length_lo & 0b0111_0000;

        let mut offset = 1;

        let length = if b_flag {
            let flags_and_length_hi = bytes[1];
            let length = u16::from_be_bytes([flags_and_length_lo, flags_and_length_hi]) as usize;
            offset += 1;
            length
        } else {
            let length = (flags_and_length_lo & 0x0F) as usize;
            length
        };

        let mut commands = Vec::new();

        let mut running_status: Option<u8> = None;
        let mut read_delta_time = (flags & 0b0010_0000) != 0;

        while offset < length {
            let (timed_command, bytes_read) =
                TimedCommand::from_be_bytes(&bytes[offset..], running_status, read_delta_time)?;
            read_delta_time = true;
            running_status = Some(timed_command.command().status);
            commands.push(timed_command);
            offset += bytes_read;
        }

        Ok(MidiCommandSection { flags, commands })
    }

    pub fn write_to_bytes(&self, bytes: &mut [u8]) -> Result<usize, std::io::Error> {
        let total_length = self.length();
        let mut offset: usize;
        if total_length > 0x000F {
            // If length > 0x0F, use 12 bits for length and 4 bits for flags
            // Set the high bit to indicate extended length
            let flags_and_length: u16 =
                0x8000 | ((self.flags as u16) << 8) | ((total_length as u16) & 0x0FFF);
            bytes[0..2].copy_from_slice(&flags_and_length.to_be_bytes());
            offset = 2;
        } else {
            // Otherwise, use 4 bits for length and 4 bits for flags
            let flags_and_length: u8 = (self.flags) | ((total_length as u8) & 0x000F);
            bytes[0] = flags_and_length;
            offset = 1;
        }

        let command_start = offset;
        let mut running_status: Option<u8> = None;
        for command in &self.commands {
            let write_delta_time = if offset == command_start {
                self.z_flag()
            } else {
                true
            };
            let bytes_written =
                command.write_to_bytes(&mut bytes[offset..], running_status, write_delta_time)?;
            running_status = Some(command.command().status);
            offset += bytes_written;
        }

        Ok(offset)
    }
}

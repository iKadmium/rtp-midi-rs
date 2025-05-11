use log::{info, trace};

use crate::midi_command::{CommandType, MidiCommand};
use crate::recovery_journal::recovery_journal::RecoveryJournal;

#[derive(Debug)]
pub struct MidiPacket {
    pub commands: Vec<(u32, MidiCommand)>, // MIDI commands with delta times and variable-length command data
    pub recovery_journal: Option<RecoveryJournal>, // Optional recovery journal
}

impl MidiPacket {
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        let mut i: usize = 0;
        let (header, header_length) = MidiPacketHeader::parse(bytes)?;

        i += header_length;

        let (commands, commands_length) =
            Self::parse_commands(&bytes[i..], header.z_flag, header.length)?;

        i += commands_length;

        let recovery_journal = if header.j_flag {
            RecoveryJournal::parse(&bytes[i..])
        } else {
            None
        };

        Some(Self {
            commands,
            recovery_journal,
        })
    }
}

#[derive(Debug)]
pub struct MidiPacketHeader {
    pub version: u8,       // Version (should be 2)
    pub p_flag: bool,      // P flag (should be 0)
    pub x_flag: bool,      // X flag (should be 0)
    pub cc: u8,            // CC field (should be 0)
    pub m_flag: bool,      // M flag (should be 1)
    pub pt: u8,            // PT field (should be 0x61)
    pub timestamp: u32,    // Lower 32 bits of the timestamp in 100-microsecond units
    pub ssrc: u32,         // Sender SSRC
    pub b_flag: bool,      // B flag
    pub j_flag: bool,      // J flag (recovery journal)
    pub z_flag: bool,      // Z flag
    pub p_flag_data: bool, // P flag for data
    pub length: u16,       // Length of the MIDI command section (4 bits)
}

impl MidiPacketHeader {
    pub fn parse(bytes: &[u8]) -> Option<(Self, usize)> {
        if bytes.len() < 13 {
            return None; // Not enough data for a valid MIDI packet header
        }

        let version = (bytes[0] >> 6) & 0b11;
        let p_flag = (bytes[0] & 0b0010_0000) != 0;
        let x_flag = (bytes[0] & 0b0001_0000) != 0;
        let cc = bytes[0] & 0b0000_1111;
        let m_flag = (bytes[1] & 0b1000_0000) != 0;
        let pt = bytes[1] & 0b0111_1111;

        if version != 2 || p_flag || x_flag || cc != 0 || pt != 0x61 {
            return None; // Invalid header fields
        }

        let timestamp = u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        let ssrc = u32::from_be_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);

        let b_flag = (bytes[12] & 0b1000_0000) != 0;
        let j_flag = (bytes[12] & 0b0100_0000) != 0;
        let z_flag = (bytes[12] & 0b0010_0000) != 0;
        let p_flag_data = (bytes[12] & 0b0001_0000) != 0;
        let length = if b_flag {
            let len_high = (bytes[12] & 0b0000_1111) as u16;
            if bytes.len() < 14 {
                return None;
            }
            let len_low = bytes[13] as u16;
            (len_high << 8) | len_low
        } else {
            (bytes[12] & 0b0000_1111) as u16
        };

        let header_length = if b_flag { 14 } else { 13 };
        Some((
            Self {
                version,
                p_flag,
                x_flag,
                cc,
                m_flag,
                pt,
                timestamp,
                ssrc,
                b_flag,
                j_flag,
                z_flag,
                p_flag_data,
                length,
            },
            header_length,
        ))
    }
}

impl MidiPacket {
    fn parse_commands(
        bytes: &[u8],
        z_flag: bool,
        length: u16,
    ) -> Option<(Vec<(u32, MidiCommand)>, usize)> {
        let mut commands = Vec::new();
        let mut i: usize = 0;

        if length > 3 {
            info!("About to read {} bytes", length);
        } else {
            trace!("About to read {} bytes", length);
        }

        let mut running_status: CommandType = CommandType::Unknown;

        while i < length as usize {
            let delta_time = if i > 0 || z_flag {
                let (delta_time, delta_time_length) = Self::parse_delta_time(&bytes[i..]);
                i += delta_time_length;
                trace!("Delta time: {}, {} bytes", delta_time, delta_time_length);
                delta_time
            } else {
                0
            };

            if i >= bytes.len() {
                return None;
            }

            let command_bytes = bytes[i..].to_vec();
            let (command, command_length) =
                MidiCommand::from_bytes(&command_bytes, running_status)?;
            running_status = command.command;

            i += command_length;
            commands.push((delta_time, command));
        }

        Some((commands, i))
    }

    fn parse_delta_time(bytes: &[u8]) -> (u32, usize) {
        let mut delta_time = 0u32;
        let mut i = 0;

        loop {
            let byte = bytes[i];
            i += 1;
            delta_time = (delta_time << 7) | (byte & 0b0111_1111) as u32;
            if byte & 0b1000_0000 == 0 {
                break;
            }
        }

        (delta_time, i)
    }
}

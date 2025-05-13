use log::trace;

use crate::midi_command::{CommandType, MidiCommand};
use crate::recovery_journal::recovery_journal::RecoveryJournal;

#[derive(Debug)]
#[allow(dead_code)]
pub struct MidiPacket {
    pub commands: Vec<MidiCommand>, // MIDI commands with delta times and variable-length command data
    pub recovery_journal: Option<RecoveryJournal>, // Optional recovery journal
}

impl MidiPacket {
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        let mut i: usize = 0;
        let (header, header_length) = MidiPacketHeader::parse(bytes)?;
        i += header_length;

        let (commands, commands_length) =
            match Self::parse_commands(&bytes[i..], header.z_flag, header.length) {
                Ok((commands, length)) => (commands, length),
                Err(e) => {
                    return Err(e); // Error parsing commands
                }
            };
        i += commands_length;

        let recovery_journal = if header.j_flag {
            match RecoveryJournal::parse(&bytes[i..]) {
                Ok(journal) => Some(journal),
                Err(e) => {
                    return Err(format!("Error parsing recovery journal: {}", e));
                }
            }
        } else {
            None
        };

        Ok(Self {
            commands,
            recovery_journal,
        })
    }
}

#[derive(Debug)]
#[allow(dead_code)]
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
    pub fn parse(bytes: &[u8]) -> Result<(Self, usize), String> {
        if bytes.len() < 13 {
            return Err("Not enough data for a valid MIDI packet header".to_string());
        }

        let version = (bytes[0] >> 6) & 0b11;
        let p_flag = (bytes[0] & 0b0010_0000) != 0;
        let x_flag = (bytes[0] & 0b0001_0000) != 0;
        let cc = bytes[0] & 0b0000_1111;
        let m_flag = (bytes[1] & 0b1000_0000) != 0;
        let pt = bytes[1] & 0b0111_1111;

        if version != 2 || p_flag || x_flag || cc != 0 || pt != 0x61 {
            return Err("Invalid header fields".to_string());
        }

        let timestamp = u32::from_be_bytes(bytes[4..8].try_into().unwrap());
        let ssrc = u32::from_be_bytes(bytes[8..12].try_into().unwrap());

        let b_flag = (bytes[12] & 0b1000_0000) != 0;
        let j_flag = (bytes[12] & 0b0100_0000) != 0;
        let z_flag = (bytes[12] & 0b0010_0000) != 0;
        let p_flag_data = (bytes[12] & 0b0001_0000) != 0;
        let length = if b_flag {
            u16::from_be_bytes([bytes[12] & 0b0000_1111, bytes[13]])
        } else {
            (bytes[12] & 0b0000_1111) as u16
        };

        let header_length = if b_flag { 14 } else { 13 };
        Ok((
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
    ) -> Result<(Vec<MidiCommand>, usize), String> {
        let mut commands = Vec::new();
        let mut i: usize = 0;
        trace!("About to read {} bytes", length);

        let mut running_status: Option<CommandType> = None;
        let mut running_channel: Option<u8> = None;

        while i < length as usize {
            if i >= bytes.len() {
                return Err("Not enough data for MIDI command".to_string());
            }

            let has_delta_time = i > 0 || z_flag;

            let command_bytes = bytes[i..].to_vec();
            match MidiCommand::from_bytes(
                &command_bytes,
                has_delta_time,
                running_status,
                running_channel,
            ) {
                Ok((command, length)) => {
                    running_status = Some(command.command);
                    running_channel = Some(command.channel);
                    i += length;
                    commands.push(command);
                }
                Err(e) => {
                    return Err(e);
                }
            }
        }

        Ok((commands, i))
    }
}

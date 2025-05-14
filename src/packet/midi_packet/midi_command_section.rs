use bitstream_io::{BitRead, FromBitStream};
use log::trace;

use crate::packet::midi_packet::midi_command::CommandType;

use super::midi_command::MidiCommand;

#[derive(Debug)]
#[allow(dead_code)]
pub struct MidiCommandSection {
    pub phantom_flag: bool,
    pub has_journal: bool,
    pub commands: Vec<MidiCommand>,
}

impl FromBitStream for MidiCommandSection {
    type Error = std::io::Error;

    fn from_reader<R: BitRead + ?Sized>(reader: &mut R) -> Result<Self, Self::Error> {
        let b_flag = reader.read_bit()?;
        let j_flag = reader.read_bit()?;
        let z_flag = reader.read_bit()?;
        let p_flag = reader.read_bit()?;
        let length = if b_flag {
            reader.read::<12, u16>()?
        } else {
            reader.read::<4, u16>()?
        };

        let mut commands = Vec::new();
        trace!("About to read {} bytes", length);

        let mut running_status: Option<CommandType> = None;
        let mut running_channel: Option<u8> = None;

        let mut bytes_read: u16 = 0;

        while bytes_read < length {
            let has_delta_time = !commands.is_empty() || z_flag;
            let (command, length) =
                MidiCommand::read_command(reader, has_delta_time, running_status, running_channel)?;
            bytes_read += length as u16;
            running_channel = Some(command.channel);
            running_status = Some(command.command);
            commands.push(command);
        }

        Ok(MidiCommandSection {
            phantom_flag: p_flag,
            has_journal: j_flag,
            commands,
        })
    }
}

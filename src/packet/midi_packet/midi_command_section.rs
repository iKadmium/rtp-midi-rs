use bitstream_io::{BitRead, BitWrite, FromBitStream, ToBitStream};
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

impl ToBitStream for MidiCommandSection {
    type Error = std::io::Error;

    fn to_writer<W: BitWrite + ?Sized>(&self, writer: &mut W) -> Result<(), Self::Error> {
        let length = {
            let mut total_length = 0;

            for (index, command) in self.commands.iter().enumerate() {
                total_length += command.size() as u16;
                if index > 0 {
                    match &command.delta_time {
                        Some(delta_time) => total_length += delta_time.size() as u16,
                        None => total_length += 1, // Zero-length delta time
                    }
                }
            }
            total_length
        };
        let b_flag = length > 0xFFF;
        writer.write_bit(b_flag)?;
        writer.write_bit(self.has_journal)?;
        writer.write_bit(false)?; // z_flag
        writer.write_bit(self.phantom_flag)?; // p_flag

        if self.phantom_flag {
            writer.write::<12, u16>(length)?;
        } else {
            writer.write::<4, u16>(length)?;
        }

        let mut running_status: Option<CommandType> = None;
        let mut running_channel: Option<u8> = None;

        for (i, command) in self.commands.iter().enumerate() {
            let write_delta_time = i > 0;
            command.to_writer(writer, running_status, running_channel, write_delta_time)?;
            running_status = Some(command.command);
            running_channel = Some(command.channel);
        }

        Ok(())
    }
}

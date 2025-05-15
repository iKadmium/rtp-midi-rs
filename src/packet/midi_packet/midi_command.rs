use log::trace;
use std::io::Error;

use super::delta_time::DeltaTime;

#[derive(Clone)]
#[allow(dead_code)]
pub struct MidiCommand {
    pub delta_time: Option<DeltaTime>,
    pub command: CommandType,
    pub channel: u8,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
#[allow(dead_code)]
pub enum CommandType {
    NoteOff,
    NoteOn,
    PolyphonicKeyPressure,
    ControlChange,
    ProgramChange,
    ChannelPressure,
    PitchBend,
}

impl CommandType {
    pub fn size(&self) -> usize {
        match self {
            CommandType::NoteOff => 2,
            CommandType::NoteOn => 2,
            CommandType::PolyphonicKeyPressure => 2,
            CommandType::ControlChange => 2,
            CommandType::ProgramChange => 1,
            CommandType::ChannelPressure => 1,
            CommandType::PitchBend => 2,
        }
    }
}

impl MidiCommand {
    pub fn read_command<R: bitstream_io::BitRead + ?Sized>(
        reader: &mut R,
        has_delta_time: bool,
        running_status: Option<CommandType>,
        running_channel: Option<u8>,
    ) -> Result<(Self, usize), std::io::Error> {
        trace!("Parsing MIDI command");

        let mut bytes_read = 0;

        let delta_time = if has_delta_time {
            let (delta_time, length) = DeltaTime::from_reader(reader)?;
            bytes_read += length;
            Some(delta_time)
        } else {
            None
        };

        let status_bit = reader.read_bit()?;
        let (command, channel) = if status_bit {
            let status_nibble = (reader.read::<3, u8>()? << 4) | 0x80;
            let channel_nibble = reader.read::<4, u8>()?;
            bytes_read += 1;
            (CommandType::from(status_nibble), channel_nibble)
        } else {
            let status = running_status.ok_or_else(|| {
                Error::new(std::io::ErrorKind::InvalidInput, "Null running status")
            })?;
            let channel = running_channel.ok_or_else(|| {
                Error::new(std::io::ErrorKind::InvalidInput, "Null running channel")
            })?;
            (status, channel)
        };

        let data_length = command.size();
        let mut data = vec![0; data_length];

        if reader.byte_aligned() {
            reader.read_bytes(&mut data)?;
            bytes_read += data_length;
        } else {
            let first_byte = reader.read::<7, u8>()?;
            data[0] = first_byte;
            bytes_read += 1;
            let mut remaining_bytes = data_length - 1;
            while remaining_bytes > 0 {
                let byte = reader.read::<8, u8>()?;
                data[data_length - remaining_bytes] = byte;
                remaining_bytes -= 1;
                bytes_read += 1;
            }
        }

        Ok((
            MidiCommand {
                delta_time,
                command,
                channel,
                data,
            },
            bytes_read,
        ))
    }

    pub fn size(&self) -> usize {
        let mut size: usize = 1;
        size += self.command.size();
        size
    }

    pub(crate) fn to_writer<W: bitstream_io::BitWrite + ?Sized>(
        &self,
        writer: &mut W,
        running_status: Option<CommandType>,
        running_channel: Option<u8>,
        write_delta_time: bool,
    ) -> std::io::Result<()> {
        if write_delta_time {
            if let Some(delta_time) = &self.delta_time {
                delta_time.to_writer(writer)?;
            } else {
                writer.write::<8, _>(DeltaTime::ZERO)?;
            }
        }

        let status_bit =
            Some(self.command) != running_status || Some(self.channel) != running_channel;

        if status_bit {
            writer.write_bit(true)?;
            writer.write::<3, _>(self.command as u8 >> 4)?;
            writer.write::<4, _>(self.channel)?;
        }

        writer.write_bytes(&self.data)?;

        Ok(())
    }
}

impl From<u8> for CommandType {
    fn from(value: u8) -> Self {
        match value & 0xF0 {
            0x80 => CommandType::NoteOff,
            0x90 => CommandType::NoteOn,
            0xA0 => CommandType::PolyphonicKeyPressure,
            0xB0 => CommandType::ControlChange,
            0xC0 => CommandType::ProgramChange,
            0xD0 => CommandType::ChannelPressure,
            0xE0 => CommandType::PitchBend,
            _ => panic!("Invalid MIDI command type"),
        }
    }
}

impl std::fmt::Debug for MidiCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MidiCommand")
            .field("delta_time", &self.delta_time)
            .field("command", &self.command)
            .field("channel", &self.channel)
            .field("data", &format!("{:02X?}", self.data)) // Print data in hex
            .finish()
    }
}

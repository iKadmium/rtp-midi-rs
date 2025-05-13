use log::trace;

use crate::delta_time::DeltaTime;

#[derive(Clone)]
#[allow(dead_code)]
pub struct MidiCommand {
    pub delta_time: DeltaTime,
    pub command: CommandType,
    pub channel: u8,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Copy)]
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
            CommandType::NoteOff | CommandType::NoteOn => 2,
            CommandType::PolyphonicKeyPressure | CommandType::ControlChange => 2,
            CommandType::ProgramChange | CommandType::ChannelPressure => 1,
            CommandType::PitchBend => 2,
        }
    }
}

impl MidiCommand {
    pub fn from_bytes(
        bytes: &[u8],
        has_delta_time: bool,
        running_status: Option<CommandType>,
        running_channel: Option<u8>,
    ) -> Result<(Self, usize), String> {
        trace!("Parsing MIDI command from bytes: {:02X?}", bytes);

        let mut i = 0;
        let delta_time = if has_delta_time {
            match DeltaTime::from_bytes(bytes) {
                Ok((delta_time, length)) => {
                    i += length;
                    delta_time
                }
                Err(e) => {
                    return Err(format!("Error parsing delta time: {}", e));
                }
            }
        } else {
            DeltaTime::new(0)
        };

        let (command, channel) = if bytes[0] & 0x80 == 0 {
            let status = match running_status {
                Some(status) => status,
                None => return Err("Null running status".to_string()),
            };
            let channel = match running_channel {
                Some(channel) => channel,
                None => return Err("Null running channel".to_string()),
            };
            (status, channel)
        } else {
            i = i + 1;
            (CommandType::from(bytes[0] & 0xF0), bytes[0] & 0x0F)
        };

        let data_length = command.size();
        let data = bytes[i..data_length + i].to_vec();
        let length = data_length + i;

        Ok((
            MidiCommand {
                delta_time,
                command,
                channel,
                data,
            },
            length,
        ))
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

use log::trace;

#[derive(Clone)]
pub struct MidiCommand {
    pub command: CommandType,
    pub channel: u8,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum CommandType {
    NoteOff,
    NoteOn,
    PolyphonicKeyPressure,
    ControlChange,
    ProgramChange,
    ChannelPressure,
    PitchBend,
    Unknown, // For unknown or unsupported commands
}

impl MidiCommand {
    pub fn from_bytes(bytes: &[u8], running_status: CommandType) -> Option<(Self, usize)> {
        trace!("Parsing MIDI command from bytes: {:02X?}", bytes);

        let command_byte = bytes[0];
        let channel = command_byte & 0x0F; // Extract channel (4 bits)

        let use_running_status = command_byte & 0x80 == 0;

        let (command, start) = if use_running_status {
            // If the first bit of command_type is 0, use running_status
            (running_status, 0)
        } else {
            // Otherwise, use the command type from the byte
            (CommandType::from(command_byte & 0xF0), 1)
        };

        let data_length = match command {
            CommandType::NoteOff | CommandType::NoteOn => 2,
            CommandType::PolyphonicKeyPressure | CommandType::ControlChange => 2,
            CommandType::ProgramChange | CommandType::ChannelPressure => 1,
            CommandType::PitchBend => 2,
            CommandType::Unknown => return None, // Handle unknown commands gracefully
        };

        let data = bytes[start..(data_length + start)].to_vec();
        let length = if use_running_status {
            data_length
        } else {
            data_length + 1
        };

        Some((
            MidiCommand {
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
            _ => CommandType::Unknown, // Or handle invalid values gracefully
        }
    }
}

impl From<CommandType> for u8 {
    fn from(command: CommandType) -> Self {
        match command {
            CommandType::NoteOff => 0x80,
            CommandType::NoteOn => 0x90,
            CommandType::PolyphonicKeyPressure => 0xA0,
            CommandType::ControlChange => 0xB0,
            CommandType::ProgramChange => 0xC0,
            CommandType::ChannelPressure => 0xD0,
            CommandType::PitchBend => 0xE0,
            CommandType::Unknown => 0x00, // Or handle invalid values gracefully
        }
    }
}

impl std::fmt::Debug for MidiCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MidiCommand")
            .field("command", &self.command)
            .field("channel", &self.channel)
            .field("data", &format!("{:02X?}", self.data)) // Print data in hex
            .finish()
    }
}

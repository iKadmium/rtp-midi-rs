use log::trace;
use std::io::Error;

#[derive(Clone, PartialEq)]
#[allow(dead_code)]
pub struct MidiCommand {
    pub status: u8,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
#[allow(dead_code)]
pub enum CommandType {
    NoteOff = 0x80,
    NoteOn = 0x90,
    PolyphonicKeyPressure = 0xA0,
    ControlChange = 0xB0,
    ProgramChange = 0xC0,
    ChannelPressure = 0xD0,
    PitchBend = 0xE0,
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

impl MidiCommand {
    pub fn new(command: CommandType, channel: u8, data: Vec<u8>) -> Self {
        let status = (command as u8) | (channel & 0x0F);
        if data.len() < command.size() {
            panic!("Invalid data length for command type");
        }

        let cloned_data = data[0..command.size()].to_owned();

        MidiCommand {
            status,
            data: cloned_data,
        }
    }

    fn channel(&self) -> u8 {
        self.status & 0x0F
    }

    fn command(&self) -> CommandType {
        CommandType::from(self.status)
    }

    pub fn from_be_bytes(
        bytes: &[u8],
        running_status: Option<u8>,
    ) -> Result<(Self, usize), std::io::Error> {
        trace!("Parsing MIDI command from bytes, {:x?}", bytes);

        let mut bytes_read = 0;

        let status = if bytes[bytes_read] & 0x80 == 0 {
            match running_status {
                Some(status) => status,
                None => {
                    return Err(Error::new(
                        std::io::ErrorKind::InvalidData,
                        "No status bit and running status is not set",
                    ));
                }
            }
        } else {
            bytes_read += 1;
            bytes[bytes_read - 1]
        };

        let data_length = CommandType::from(status).size();
        let data = bytes[bytes_read..bytes_read + data_length].to_vec();
        bytes_read += data_length;

        Ok((MidiCommand { status, data }, bytes_read))
    }

    pub fn write_to_bytes(
        &self,
        bytes: &mut [u8],
        running_status: Option<u8>,
    ) -> Result<usize, std::io::Error> {
        trace!("Writing MIDI command to bytes");

        let mut bytes_written = 0;

        if self.status & 0x80 == 0 {
            if let Some(status) = running_status {
                bytes[bytes_written] = status;
            } else {
                return Err(Error::new(
                    std::io::ErrorKind::InvalidData,
                    "No status bit and running status is not set",
                ));
            }
        } else {
            bytes[bytes_written] = self.status;
            bytes_written += 1;
        }

        bytes[bytes_written..bytes_written + self.data.len()].copy_from_slice(&self.data);
        bytes_written += self.data.len();

        Ok(bytes_written)
    }
}

impl std::fmt::Debug for MidiCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MidiCommand")
            .field("command", &self.command())
            .field("channel", &self.channel())
            .field("data", &format!("{:02X?}", self.data)) // Print data in hex
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_midi_command() {
        let command = MidiCommand {
            status: 0x97,
            data: vec![0x40, 0x7F],
        };

        assert_eq!(command.channel(), 7);
        assert_eq!(command.command(), CommandType::NoteOn);
        assert_eq!(command.data, vec![0x40, 0x7F]);
    }

    #[test]
    fn test_midi_command_from_bytes_with_status_byte() {
        let bytes: [u8; 4] = [0x90, 0x40, 0x7F, 0x00];
        let (command, bytes_read) = MidiCommand::from_be_bytes(&bytes, None).unwrap();
        assert_eq!(command.status, 0x90);
        assert_eq!(command.data, vec![0x40, 0x7F]);
        assert_eq!(bytes_read, 3);
    }

    #[test]
    fn test_midi_command_from_bytes_without_status_byte() {
        let bytes: [u8; 3] = [0x40, 0x7F, 0x00];
        let running_status = Some(0x90);
        let (command, bytes_read) = MidiCommand::from_be_bytes(&bytes, running_status).unwrap();
        assert_eq!(command.status, 0x90);
        assert_eq!(command.data, vec![0x40, 0x7F]);
        assert_eq!(bytes_read, 2);
    }

    #[test]
    fn test_midi_command_write_to_bytes() {
        let command = MidiCommand {
            status: 0x90,
            data: vec![0x40, 0x7F],
        };
        let mut bytes = [0u8; 4];
        let bytes_written = command.write_to_bytes(&mut bytes, None).unwrap();
        assert_eq!(bytes_written, 3);
        assert_eq!(&bytes[..3], &[0x90, 0x40, 0x7F]);
    }

    #[test]
    fn test_serialize_and_deserialize() {
        let original_command = MidiCommand {
            status: 0x90,
            data: vec![0x40, 0x7F],
        };

        let mut bytes = [0u8; 4];
        let bytes_written = original_command.write_to_bytes(&mut bytes, None).unwrap();

        let (deserialized_command, bytes_read) =
            MidiCommand::from_be_bytes(&bytes[..bytes_written], None).unwrap();

        assert_eq!(original_command, deserialized_command);
        assert_eq!(bytes_written, bytes_read);
    }
}

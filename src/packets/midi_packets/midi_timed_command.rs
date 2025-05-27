use std::io::{Read, Write};
use tracing::{Level, event};

use super::{delta_time::ReadDeltaTimeExt, delta_time::WriteDeltaTimeExt, midi_command::MidiCommand};

#[derive(Debug, Clone, PartialEq)]
pub struct TimedCommand {
    delta_time: Option<u32>,
    command: MidiCommand,
}

impl TimedCommand {
    pub fn new(delta_time: Option<u32>, command: MidiCommand) -> Self {
        TimedCommand { delta_time, command }
    }

    pub fn delta_time(&self) -> Option<u32> {
        self.delta_time
    }

    pub fn command(&self) -> &MidiCommand {
        &self.command
    }

    pub(super) fn read<R: Read>(reader: &mut R, running_status: Option<u8>, has_delta_time: bool) -> Result<Self, std::io::Error> {
        event!(Level::TRACE, "Parsing TimedCommand from reader");
        let delta_time = if has_delta_time { Some(reader.read_delta_time()?) } else { None };
        let command = MidiCommand::read(reader, running_status)?;
        Ok(TimedCommand { delta_time, command })
    }

    pub(super) fn write<W: Write>(&self, writer: &mut W, running_status: Option<u8>, write_delta_time: bool) -> Result<usize, std::io::Error> {
        let mut bytes_written = 0;

        if write_delta_time {
            match self.delta_time {
                Some(dt) => bytes_written += writer.write_delta_time(dt)?,
                None => bytes_written += writer.write_delta_time(0)?,
            }
        }

        bytes_written += self.command.write(writer, running_status)?;

        Ok(bytes_written)
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn test_timed_command() {
        let delta_time = 0x123456;
        let command = MidiCommand::NoteOn {
            channel: 7,
            key: 0x40,
            velocity: 0x7F,
        };
        let timed_command = TimedCommand {
            delta_time: Some(delta_time),
            command: command.clone(),
        };

        assert_eq!(timed_command.delta_time().unwrap(), delta_time);
        assert_eq!(timed_command.command(), &command);
    }

    #[test]
    fn test_timed_command_read_with_delta_time() {
        let mut bytes = Vec::<u8>::new();
        let command = MidiCommand::NoteOn {
            channel: 7,
            key: 0x40,
            velocity: 0x7F,
        };
        let delta_time = 0x123456;

        bytes.write_delta_time(delta_time).unwrap();
        command.write(&mut bytes, None).unwrap();

        let mut cursor = Cursor::new(bytes);

        let timed_command = TimedCommand::read(&mut cursor, None, true).unwrap();

        assert_eq!(timed_command.command(), &command);
        assert_eq!(timed_command.delta_time(), Some(delta_time));
    }

    #[test]
    fn test_timed_command_from_bytes_without_delta_time() {
        let mut bytes = Vec::<u8>::new();
        let command = MidiCommand::NoteOn {
            channel: 7,
            key: 0x40,
            velocity: 0x7F,
        };

        command.write(&mut bytes, None).unwrap();

        let mut cursor = Cursor::new(bytes);

        let timed_command = TimedCommand::read(&mut cursor, None, false).unwrap();

        assert_eq!(timed_command.command(), &command);
        assert_eq!(timed_command.delta_time(), None);
    }

    #[test]
    fn test_timed_command_write() {
        let mut expected_bytes = vec![0; 10];

        let delta_time = 0x123456;
        expected_bytes.write_delta_time(delta_time).unwrap();
        let command = MidiCommand::NoteOn {
            channel: 7,
            key: 0x40,
            velocity: 0x7F,
        };
        command.write(&mut expected_bytes, None).unwrap();

        let timed_command = TimedCommand {
            delta_time: Some(delta_time),
            command: command.clone(),
        };

        let mut bytes = vec![0u8; 10];
        let bytes_written = timed_command.write(&mut bytes, None, true).unwrap();

        assert_eq!(bytes[..bytes_written], expected_bytes[..bytes_written]);
    }

    #[test]
    fn test_timed_command_write_without_delta_time() {
        let mut expected_bytes = Vec::<u8>::new();
        let mut expected_bytes_written = 0;

        let command = MidiCommand::NoteOn {
            channel: 7,
            key: 0x40,
            velocity: 0x7F,
        };
        expected_bytes_written += command.write(&mut expected_bytes, None).unwrap();

        let timed_command = TimedCommand {
            delta_time: None,
            command: command.clone(),
        };

        let mut bytes = Vec::<u8>::new();
        let bytes_written = timed_command.write(&mut bytes, None, false).unwrap();

        assert_eq!(bytes_written, expected_bytes_written);
        assert_eq!(bytes[..bytes_written], expected_bytes[..bytes_written]);
    }

    #[test]
    fn test_timed_command_write_with_zero_delta_time() {
        let mut expected_bytes = vec![0; 10];
        let mut expected_bytes_written = 0;

        let delta_time = 0;
        expected_bytes_written += expected_bytes.write_delta_time(delta_time).unwrap();

        let command = MidiCommand::NoteOn {
            channel: 7,
            key: 0x40,
            velocity: 0x7F,
        };
        expected_bytes_written += command.write(&mut expected_bytes, None).unwrap();

        let timed_command = TimedCommand {
            delta_time: None,
            command: command.clone(),
        };

        let mut bytes = vec![0u8; 10];
        let bytes_written = timed_command.write(&mut bytes, None, true).unwrap();

        assert_eq!(bytes_written, expected_bytes_written);
        assert_eq!(bytes[..bytes_written], expected_bytes[..bytes_written]);
    }

    #[test]
    fn test_timed_command_serialize_and_deserialize() {
        let delta_time = 0x123456;
        let command = MidiCommand::NoteOn {
            channel: 7,
            key: 0x40,
            velocity: 0x7F,
        };
        let original_timed_command = TimedCommand {
            delta_time: Some(delta_time),
            command: command.clone(),
        };

        let mut bytes = Vec::new();
        let _bytes_written = original_timed_command.write(&mut bytes, None, true).unwrap();

        let mut cursor = Cursor::new(bytes);

        let deserialized_timed_command = TimedCommand::read(&mut cursor, None, true).unwrap();

        assert_eq!(original_timed_command, deserialized_timed_command);
    }
}

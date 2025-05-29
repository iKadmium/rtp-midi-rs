use std::io::Write;

use super::{delta_time::WriteDeltaTimeExt, midi_command::MidiCommand};

#[derive(Debug, Clone, PartialEq)]
pub struct TimedCommand<'a> {
    delta_time: Option<u32>,
    command: MidiCommand<'a>,
}

impl<'a> TimedCommand<'a> {
    pub fn new(delta_time: Option<u32>, command: MidiCommand<'a>) -> Self {
        TimedCommand { delta_time, command }
    }

    pub fn delta_time(&self) -> u32 {
        self.delta_time.unwrap_or(0)
    }

    pub fn command(&self) -> &MidiCommand {
        &self.command
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

        assert_eq!(timed_command.delta_time(), delta_time);
        assert_eq!(timed_command.command(), &command);
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
}

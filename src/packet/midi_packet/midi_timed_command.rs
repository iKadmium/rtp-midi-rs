use log::trace;

use super::{delta_time::DeltaTime, midi_command::MidiCommand};

#[derive(Debug, Clone, PartialEq)]
pub struct TimedCommand {
    delta_time: Option<DeltaTime>,
    command: MidiCommand,
}

impl TimedCommand {
    pub fn delta_time(&self) -> Option<&DeltaTime> {
        self.delta_time.as_ref()
    }

    pub fn command(&self) -> &MidiCommand {
        &self.command
    }

    pub fn from_be_bytes(
        bytes: &[u8],
        running_status: Option<u8>,
        has_delta_time: bool,
    ) -> Result<(Self, usize), std::io::Error> {
        trace!("Parsing TimedCommand from bytes, {:x?}", bytes);
        let mut bytes_read = 0;
        let delta_time = if has_delta_time {
            let delta_time = DeltaTime::from_be_bytes(bytes)?;
            bytes_read += delta_time.size();
            Some(delta_time)
        } else {
            None
        };

        let (command, length) = MidiCommand::from_be_bytes(&bytes[bytes_read..], running_status)?;
        bytes_read += length;

        Ok((
            TimedCommand {
                delta_time,
                command,
            },
            bytes_read,
        ))
    }

    pub fn write_to_bytes(
        &self,
        bytes: &mut [u8],
        running_status: Option<u8>,
        write_delta_time: bool,
    ) -> Result<usize, std::io::Error> {
        let mut bytes_written = 0;

        if write_delta_time {
            if let Some(ref delta_time) = self.delta_time {
                bytes_written += delta_time.write_to_bytes(&mut bytes[bytes_written..])?;
            } else {
                bytes_written += DeltaTime::zero().write_to_bytes(&mut bytes[bytes_written..])?;
            }
        }

        bytes_written += self
            .command
            .write_to_bytes(&mut bytes[bytes_written..], running_status)?;

        Ok(bytes_written)
    }
}

#[cfg(test)]
mod tests {
    use crate::packet::midi_packet::midi_command::CommandType;

    use super::*;

    #[test]
    fn test_timed_command() {
        let delta_time = DeltaTime::new(0x123456);
        let command = MidiCommand::new(CommandType::NoteOn, 7, vec![0x40, 0x7F]);
        let timed_command = TimedCommand {
            delta_time: Some(delta_time.clone()),
            command: command.clone(),
        };

        assert_eq!(timed_command.delta_time().unwrap(), &delta_time);
        assert_eq!(timed_command.command(), &command);
    }

    #[test]
    fn test_timed_command_from_bytes_with_delta_time() {
        let bytes: [u8; 5] = [0x00, 0x90, 0x40, 0x7F, 0x00];

        let delta_time = DeltaTime::from_be_bytes(&bytes[0..1]).unwrap();
        let (command, _command_bytes_read) = MidiCommand::from_be_bytes(&bytes[1..], None).unwrap();

        let (timed_command, timed_bytes_read) =
            TimedCommand::from_be_bytes(&bytes, None, true).unwrap();

        assert_eq!(timed_command.command(), &command);
        assert_eq!(timed_command.delta_time(), Some(&delta_time));
        assert_eq!(timed_bytes_read, 4);
    }

    #[test]
    fn test_timed_command_from_bytes_without_delta_time() {
        let bytes: [u8; 4] = [0x90, 0x40, 0x7F, 0x00];

        let (command, _command_bytes_read) = MidiCommand::from_be_bytes(&bytes[0..], None).unwrap();

        let (timed_command, timed_bytes_read) =
            TimedCommand::from_be_bytes(&bytes, None, false).unwrap();

        assert_eq!(timed_command.command(), &command);
        assert_eq!(timed_command.delta_time(), None);
        assert_eq!(timed_bytes_read, 3);
    }

    #[test]
    fn test_timed_command_write_to_bytes() {
        let mut expected_bytes = vec![0; 10];
        let mut expected_bytes_written = 0;

        let delta_time = DeltaTime::new(0x123456);
        expected_bytes_written += delta_time.write_to_bytes(&mut expected_bytes).unwrap();
        let command = MidiCommand::new(CommandType::NoteOn, 7, vec![0x40, 0x7F]);
        expected_bytes_written += command
            .write_to_bytes(&mut expected_bytes[expected_bytes_written..], None)
            .unwrap();

        let timed_command = TimedCommand {
            delta_time: Some(delta_time.clone()),
            command: command.clone(),
        };

        let mut bytes = [0u8; 10];
        let bytes_written = timed_command
            .write_to_bytes(&mut bytes, None, true)
            .unwrap();

        assert_eq!(bytes_written, expected_bytes_written);
        assert_eq!(bytes[..bytes_written], expected_bytes[..bytes_written]);
    }

    #[test]
    fn test_timed_command_write_to_bytes_without_delta_time() {
        let mut expected_bytes = vec![0; 10];
        let mut expected_bytes_written = 0;

        let command = MidiCommand::new(CommandType::NoteOn, 7, vec![0x40, 0x7F]);
        expected_bytes_written += command
            .write_to_bytes(&mut expected_bytes[expected_bytes_written..], None)
            .unwrap();

        let timed_command = TimedCommand {
            delta_time: None,
            command: command.clone(),
        };

        let mut bytes = [0u8; 10];
        let bytes_written = timed_command
            .write_to_bytes(&mut bytes, None, false)
            .unwrap();

        assert_eq!(bytes_written, expected_bytes_written);
        assert_eq!(bytes[..bytes_written], expected_bytes[..bytes_written]);
    }

    #[test]
    fn test_timed_command_write_to_bytes_with_zero_delta_time() {
        let mut expected_bytes = vec![0; 10];
        let mut expected_bytes_written = 0;

        let delta_time = DeltaTime::zero();
        expected_bytes_written += delta_time.write_to_bytes(&mut expected_bytes).unwrap();

        let command = MidiCommand::new(CommandType::NoteOn, 7, vec![0x40, 0x7F]);
        expected_bytes_written += command
            .write_to_bytes(&mut expected_bytes[expected_bytes_written..], None)
            .unwrap();

        let timed_command = TimedCommand {
            delta_time: None,
            command: command.clone(),
        };

        let mut bytes = [0u8; 10];
        let bytes_written = timed_command
            .write_to_bytes(&mut bytes, None, true)
            .unwrap();

        assert_eq!(bytes_written, expected_bytes_written);
        assert_eq!(bytes[..bytes_written], expected_bytes[..bytes_written]);
    }

    #[test]
    fn test_timed_command_serialize_and_deserialize() {
        let delta_time = DeltaTime::new(0x123456);
        let command = MidiCommand::new(CommandType::NoteOn, 7, vec![0x40, 0x7F]);
        let original_timed_command = TimedCommand {
            delta_time: Some(delta_time.clone()),
            command: command.clone(),
        };

        let mut bytes = [0u8; 10];
        let bytes_written = original_timed_command
            .write_to_bytes(&mut bytes, None, true)
            .unwrap();

        let (deserialized_timed_command, bytes_read) =
            TimedCommand::from_be_bytes(&bytes[..bytes_written], None, true).unwrap();

        assert_eq!(original_timed_command, deserialized_timed_command);
        assert_eq!(bytes_written, bytes_read);
    }
}

use bytes::BytesMut;

use crate::packets::midi_packets::delta_time::read_delta_time;

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

    pub fn from_be_bytes(bytes: &'a [u8], should_read_delta_time: bool, running_status: Option<u8>) -> std::io::Result<(Self, &'a [u8])> {
        let mut delta_time = None;

        let mut bytes = bytes;
        if should_read_delta_time {
            let (dt, new_bytes) = read_delta_time(bytes)?;
            delta_time = Some(dt);
            bytes = new_bytes;
        }

        let (command, offset) = MidiCommand::from_be_bytes(bytes, running_status)?;

        Ok((TimedCommand { delta_time, command }, offset))
    }

    pub(super) fn write(&self, bytes: &mut BytesMut, running_status: Option<u8>, write_delta_time: bool) {
        if write_delta_time {
            match self.delta_time {
                Some(dt) => bytes.write_delta_time(dt),
                None => bytes.write_delta_time(0),
            }
        }

        self.command.write(bytes, running_status);
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
        let mut expected_bytes = BytesMut::with_capacity(10);

        let delta_time = 0x123456;
        expected_bytes.write_delta_time(delta_time);
        let command = MidiCommand::NoteOn {
            channel: 7,
            key: 0x40,
            velocity: 0x7F,
        };
        command.write(&mut expected_bytes, None);

        let timed_command = TimedCommand {
            delta_time: Some(delta_time),
            command: command.clone(),
        };

        let mut bytes = BytesMut::with_capacity(10);
        timed_command.write(&mut bytes, None, true);

        assert_eq!(bytes[..], expected_bytes[..]);
    }

    #[test]
    fn test_timed_command_write_without_delta_time() {
        let mut expected_bytes = BytesMut::with_capacity(10);

        let command = MidiCommand::NoteOn {
            channel: 7,
            key: 0x40,
            velocity: 0x7F,
        };
        command.write(&mut expected_bytes, None);

        let timed_command = TimedCommand {
            delta_time: None,
            command: command.clone(),
        };

        let mut bytes = BytesMut::with_capacity(10);
        timed_command.write(&mut bytes, None, false);

        assert_eq!(bytes[..], expected_bytes[..]);
    }

    #[test]
    fn test_timed_command_write_with_zero_delta_time() {
        let mut expected_bytes = BytesMut::with_capacity(10);

        let delta_time = 0;
        expected_bytes.write_delta_time(delta_time);

        let command = MidiCommand::NoteOn {
            channel: 7,
            key: 0x40,
            velocity: 0x7F,
        };
        command.write(&mut expected_bytes, None);

        let timed_command = TimedCommand {
            delta_time: None,
            command: command.clone(),
        };

        let mut bytes = BytesMut::with_capacity(10);
        timed_command.write(&mut bytes, None, true);

        assert_eq!(bytes[..], expected_bytes[..]);
    }
}

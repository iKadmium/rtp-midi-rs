use super::{delta_time::WriteDeltaTimeExt, midi_timed_command::TimedCommand};
use std::io::Write;

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub struct MidiCommandListBody {
    commands: Vec<TimedCommand>,
}

impl MidiCommandListBody {
    pub fn new(commands: &[TimedCommand]) -> Self {
        MidiCommandListBody { commands: commands.to_vec() }
    }

    pub fn size(&self, z_flag: bool) -> usize {
        let mut length: usize = 0;
        let mut running_status: Option<u8> = None;
        for (i, command) in self.commands.iter().enumerate() {
            if i > 0 || z_flag {
                length += <Vec<u8> as WriteDeltaTimeExt>::delta_time_size(command.delta_time())
            }
            if Some(command.command().status()) != running_status {
                length += 1;
            }
            length += command.command().size();
            running_status = Some(command.command().status());
        }

        if length > 0x0F {
            length += 1; // Extra byte for big header
        }

        length
    }

    pub fn commands(&self) -> &[TimedCommand] {
        &self.commands
    }

    pub fn write<W: Write>(&self, writer: &mut W, z_flag: bool) -> Result<usize, std::io::Error> {
        let mut offset = 0;
        let mut running_status: Option<u8> = None;
        for command in &self.commands {
            let write_delta_time = if offset == 0 { z_flag } else { true };
            let bytes_written = command.write(writer, running_status, write_delta_time)?;
            running_status = Some(command.command().status());
            offset += bytes_written;
        }

        Ok(offset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packets::midi_packets::midi_command::MidiCommand;
    use crate::packets::midi_packets::midi_timed_command::TimedCommand;

    #[test]
    fn test_new_and_commands() {
        let command = MidiCommand::NoteOn {
            channel: 1,
            key: 60,
            velocity: 100,
        };
        let timed_command = TimedCommand::new(None, command);
        let body = MidiCommandListBody::new(&[timed_command.clone()]);
        let commands = body.commands();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0], timed_command);
    }
}

use std::io::{Read, Write};

use log::trace;

use super::midi_timed_command::TimedCommand;

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub struct MidiCommandListBody {
    commands: Vec<TimedCommand>,
}

impl MidiCommandListBody {
    pub fn new(commands: &[TimedCommand]) -> Self {
        MidiCommandListBody {
            commands: commands.to_vec(),
        }
    }

    pub fn size(&self, z_flag: bool) -> usize {
        let mut length: usize = 0;
        let mut running_status: Option<u8> = None;
        for (i, command) in self.commands.iter().enumerate() {
            if i > 0 || z_flag {
                match command.delta_time() {
                    Some(ref delta_time) => length += delta_time.size(),
                    None => {
                        length += 1;
                    }
                }
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

        return length;
    }

    pub fn commands(&self) -> &[TimedCommand] {
        &self.commands
    }

    pub fn read<R: Read>(reader: &mut R, z_flag: bool) -> Result<Self, std::io::Error> {
        trace!("Parsing MIDI command list from reader");
        let mut commands = Vec::new();

        let mut running_status: Option<u8> = None;
        let mut read_delta_time = z_flag;
        loop {
            match TimedCommand::read(reader, running_status, read_delta_time) {
                Ok(timed_command) => {
                    read_delta_time = true;
                    running_status = Some(timed_command.command().status());
                    commands.push(timed_command);
                }
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::UnexpectedEof {
                        break;
                    } else {
                        return Err(e);
                    }
                }
            }
        }

        Ok(MidiCommandListBody { commands })
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
    use crate::packet::midi_packets::delta_time::DeltaTime;
    use crate::packet::midi_packets::midi_command::MidiCommand;
    use crate::packet::midi_packets::midi_timed_command::TimedCommand;

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

    #[test]
    fn test_size_and_write_read_roundtrip() {
        let command1 = MidiCommand::NoteOn {
            channel: 1,
            key: 60,
            velocity: 100,
        };
        let command2 = MidiCommand::NoteOff {
            channel: 1,
            key: 60,
            velocity: 0,
        };
        let timed1 = TimedCommand::new(None, command1);
        let timed2 = TimedCommand::new(Some(DeltaTime::zero()), command2);
        let body = MidiCommandListBody::new(&[timed1.clone(), timed2.clone()]);
        let size = body.size(false);
        let mut buf = Vec::with_capacity(size);
        body.write(&mut buf, false).unwrap();
        let mut cursor = std::io::Cursor::new(buf);
        let parsed = MidiCommandListBody::read(&mut cursor, false).unwrap();
        assert_eq!(parsed.commands().len(), 2);
        assert_eq!(parsed.commands()[0], timed1);
        assert_eq!(parsed.commands()[1], timed2);
    }
}

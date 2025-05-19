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

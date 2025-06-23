use bytes::BytesMut;

use crate::packets::midi_packets::delta_time::delta_time_size;

use super::midi_event::MidiEvent;

#[derive(Debug, Clone, PartialEq)]
pub struct MidiCommandListBody<'a> {
    commands: &'a [MidiEvent<'a>],
}

impl<'a> MidiCommandListBody<'a> {
    pub fn new(commands: &'a [MidiEvent<'a>]) -> Self {
        Self { commands }
    }

    pub fn write(&self, buffer: &mut BytesMut, z_flag: bool) {
        let mut write_delta_time = z_flag;
        let mut running_status: Option<u8> = None;
        for command in self.commands {
            command.write(buffer, running_status, write_delta_time);
            running_status = Some(command.command().status());
            write_delta_time = true;
        }
    }

    pub fn size(&self, z_flag: bool) -> usize {
        let mut length: usize = 0;
        let mut running_status: Option<u8> = None;
        for (i, command) in self.commands.iter().enumerate() {
            if i > 0 || z_flag {
                length += delta_time_size(command.delta_time())
            }
            if Some(command.command().status()) != running_status {
                length += 1;
            }
            length += command.command().size();
            running_status = Some(command.command().status());
        }

        length
    }
}

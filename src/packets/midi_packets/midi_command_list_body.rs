use bytes::{Bytes, BytesMut};

use crate::packets::midi_packets::delta_time::delta_time_size;

use super::midi_timed_command::TimedCommand;

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub struct MidiCommandListBody<'a> {
    commands: &'a [TimedCommand<'a>],
}

impl<'a> MidiCommandListBody<'a> {
    pub fn new_as_bytes(commands: &'a [TimedCommand], z_flag: bool) -> Bytes {
        let mut buffer = BytesMut::with_capacity(Self::size(commands, false));

        let mut write_delta_time = z_flag;
        let mut running_status: Option<u8> = None;
        for command in commands {
            command.write(&mut buffer, running_status, write_delta_time);
            running_status = Some(command.command().status());
            write_delta_time = true;
        }

        buffer.freeze()
    }

    pub fn size(commands: &[TimedCommand], z_flag: bool) -> usize {
        let mut length: usize = 0;
        let mut running_status: Option<u8> = None;
        for (i, command) in commands.iter().enumerate() {
            if i > 0 || z_flag {
                length += delta_time_size(command.delta_time())
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
}

use std::io::{Cursor, Write};

use super::{
    midi_command_list_body::MidiCommandListBody,
    midi_command_list_header::{MidiCommandListFlags, MidiCommandListHeader},
    midi_packet_header::MidiPacketHeader,
    midi_timed_command::TimedCommand,
};

#[derive(Debug)]
#[allow(dead_code)]
pub struct MidiPacketBuilder {
    header: MidiPacketHeader,
    command_list: MidiCommandListBody,
    //recovery_journal: Option<RecoveryJournal>,
}

impl MidiPacketBuilder {
    pub(crate) fn new(sequence_number: u16, timestamp: u32, ssrc: u32, commands: &[TimedCommand]) -> Self {
        MidiPacketBuilder {
            header: MidiPacketHeader::new(sequence_number, timestamp, ssrc),
            command_list: MidiCommandListBody::new(commands),
            //recovery_journal: None,
        }
    }

    pub(crate) fn write<W: Write>(&self, writer: &mut W, z_flag: bool) -> std::io::Result<usize> {
        let mut bytes_written = self.header.write(writer)?;
        let command_section_header = MidiCommandListHeader::build_for(&self.command_list, false, z_flag, false);
        bytes_written += command_section_header.write(writer)?;
        bytes_written += self.command_list.write(writer, z_flag)?;
        Ok(bytes_written)
    }

    pub(crate) fn to_bytes(&self, z_flag: bool) -> Vec<u8> {
        let mut buffer = vec![0; self.size(z_flag)];
        self.write(&mut Cursor::new(&mut buffer), z_flag).expect("Failed to write MidiPacket");
        buffer
    }

    pub(crate) fn size(&self, z_flag: bool) -> usize {
        let command_section_size = self.command_list.size(z_flag);
        let needs_b_flag = MidiCommandListFlags::needs_b_flag(command_section_size);
        let command_section_header_size = MidiCommandListHeader::size(needs_b_flag);
        size_of::<MidiPacketHeader>() + command_section_header_size + command_section_size
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packets::midi_packets::midi_command::MidiCommand;
    use crate::packets::midi_packets::midi_packet_zero_alloc::MidiPacketZeroAlloc;
    use crate::packets::midi_packets::midi_timed_command::TimedCommand;

    #[test]
    fn test_midi_packet() {
        let sequence_number = 1;
        let timestamp = 1234567890;
        let ssrc = 987654321;

        let command = MidiCommand::NoteOn {
            key: 60,
            velocity: 127,
            channel: 0,
        };
        let timed_command = TimedCommand::new(None, command);
        let timed_comands = &[timed_command];

        let packet = MidiPacketBuilder::new(sequence_number, timestamp, ssrc, timed_comands);

        let mut bytes = vec![0; packet.size(false)];
        packet.write(&mut Cursor::new(&mut bytes), false).unwrap();

        let parsed_packet = MidiPacketZeroAlloc::new(bytes.as_slice()).expect("Failed to parse MIDI packet");

        assert_eq!(sequence_number, parsed_packet.sequence_number());
        assert_eq!(timestamp, parsed_packet.timestamp());
        let parsed_commands: Vec<_> = parsed_packet.commands().collect();
        assert_eq!(timed_comands.len(), parsed_commands.len());
        assert_eq!(timed_comands.first().unwrap().command(), &parsed_commands.first().unwrap().command().to_owned());
        assert_eq!(timed_comands.first().unwrap().delta_time(), parsed_commands.first().unwrap().delta_time());
    }
}

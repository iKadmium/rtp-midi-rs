use bytes::{BufMut, Bytes, BytesMut};
use zerocopy::{
    IntoBytes, KnownLayout, Unaligned,
    network_endian::{U16, U32},
};

use crate::packets::midi_packets::midi_command_list_header::{MidiCommandListFlags, MidiCommandListHeader};

use super::{midi_command_list_body::MidiCommandListBody, midi_packet_header::MidiPacketHeader, midi_timed_command::TimedCommand};

#[repr(C, packed)]
#[allow(dead_code)]
#[derive(KnownLayout, IntoBytes, Unaligned)]
pub struct MidiPacketBuilder<'a> {
    header: MidiPacketHeader,
    command_list: MidiCommandListBody<'a>,
    //recovery_journal: Option<RecoveryJournal>,
}

impl<'a> MidiPacketBuilder<'a> {
    pub(crate) fn new_as_bytes(sequence_number: U16, timestamp: U32, ssrc: U32, commands: &'a [TimedCommand]) -> Bytes {
        let header = MidiPacketHeader::new(sequence_number, timestamp, ssrc);
        let command_list_body = MidiCommandListBody::new_as_bytes(commands, false);
        let b_flag = MidiCommandListFlags::needs_b_flag(command_list_body.len());
        let flags = MidiCommandListFlags::new(b_flag, false, false, false);
        let command_list_header = MidiCommandListHeader::new(flags, command_list_body.len());

        let mut buffer = BytesMut::with_capacity(std::mem::size_of::<MidiPacketHeader>() + command_list_body.len());
        buffer.put_slice(header.as_bytes());
        command_list_header.write(&mut buffer);
        buffer.put_slice(command_list_body.as_bytes());
        buffer.freeze()
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
        let sequence_number = U16::new(1);
        let timestamp = 1234567890;
        let ssrc = 987654321;

        let command = MidiCommand::NoteOn {
            key: 60,
            velocity: 127,
            channel: 0,
        };
        let timed_command = TimedCommand::new(None, command);
        let timed_comands = &[timed_command];

        let packet = MidiPacketBuilder::new_as_bytes(sequence_number, U32::new(timestamp), U32::new(ssrc), timed_comands);
        let parsed_packet = MidiPacketZeroAlloc::new(&packet).expect("Failed to parse MIDI packet");

        assert_eq!(sequence_number, parsed_packet.sequence_number());
        assert_eq!(timestamp, parsed_packet.timestamp().get());
        let parsed_commands: Vec<_> = parsed_packet.commands().collect();
        assert_eq!(timed_comands.len(), parsed_commands.len());
        assert_eq!(timed_comands.first().unwrap().command(), &parsed_commands.first().unwrap().command().to_owned());
        assert_eq!(timed_comands.first().unwrap().delta_time(), parsed_commands.first().unwrap().delta_time());
    }
}

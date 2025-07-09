use bytes::{BufMut, Bytes, BytesMut};
use zerocopy::{
    FromBytes, Immutable, IntoBytes, KnownLayout,
    network_endian::{U16, U32},
};

use super::midi_command_iterator::MidiCommandIterator;
use super::midi_command_list_body::MidiEventList;
use crate::packets::midi_packets::{midi_command_list_header::MidiCommandListHeader, midi_event::MidiEvent, midi_packet_header::MidiPacketHeader};

#[derive(FromBytes, KnownLayout, Immutable, Debug)]
#[repr(C)]
pub(crate) struct MidiPacket {
    header: MidiPacketHeader,
    body: [u8],
}

impl MidiPacket {
    pub(crate) fn new_as_bytes(sequence_number: U16, timestamp: U32, ssrc: U32, commands: &[MidiEvent], z_flag: bool) -> Bytes {
        let packet_header = MidiPacketHeader::new(sequence_number, timestamp, ssrc);
        let command_list_header = MidiCommandListHeader::build_for(commands, z_flag);

        // Get the size of the body from the header as it's already calculated
        let mut buffer = BytesMut::with_capacity(std::mem::size_of::<MidiPacketHeader>() + command_list_header.size() + command_list_header.length());
        buffer.put_slice(packet_header.as_bytes());
        command_list_header.write(&mut buffer);
        commands.write(&mut buffer, z_flag);
        buffer.freeze()
    }

    pub fn commands(&self) -> MidiCommandIterator {
        MidiCommandIterator::new(&self.body)
    }

    pub fn sequence_number(&self) -> U16 {
        self.header.sequence_number
    }

    #[allow(dead_code)]
    pub fn timestamp(&self) -> U32 {
        self.header.timestamp
    }

    #[allow(dead_code)]
    pub fn ssrc(&self) -> U32 {
        self.header.ssrc
    }
}

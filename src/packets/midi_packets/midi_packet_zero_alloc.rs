use bytes::{BufMut, Bytes, BytesMut};
use zerocopy::{
    network_endian::{U16, U32}, FromBytes, IntoBytes
};

use super::midi_command_iterator::MidiCommandIterator;
use crate::packets::midi_packets::{midi_command_list_body::MidiCommandListBody, midi_command_list_header::{MidiCommandListFlags, MidiCommandListHeader}, midi_event::MidiEvent, midi_packet_header::MidiPacketHeader};

#[derive(Debug)]
pub(crate) struct MidiPacket<'a> {
    header: &'a MidiPacketHeader,
    body: &'a [u8],
}

impl<'a> MidiPacket<'a> {
    pub fn new(data: &'a [u8]) -> std::io::Result<Self> {
        let (header, body) = MidiPacketHeader::ref_from_prefix(data)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, format!("Failed to parse MIDI packet header: {}", e)))?;

        Ok(Self { header, body })
    }

    pub(crate) fn new_as_bytes(sequence_number: U16, timestamp: U32, ssrc: U32, commands: &'a [MidiEvent]) -> Bytes {
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

    pub fn commands(&self) -> MidiCommandIterator<'a> {
        MidiCommandIterator::new(self.body)
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

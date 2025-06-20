use zerocopy::{
    FromBytes,
    network_endian::{U16, U32},
};

use super::midi_command_iterator::MidiCommandIterator;
use crate::packets::midi_packets::midi_packet_header::MidiPacketHeader;

#[derive(Debug)]
pub(crate) struct MidiPacketZeroAlloc<'a> {
    header: &'a MidiPacketHeader,
    data: &'a [u8],
}

impl<'a> MidiPacketZeroAlloc<'a> {
    pub fn new(data: &'a [u8]) -> std::io::Result<Self> {
        let (header, body) = MidiPacketHeader::ref_from_prefix(data)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, format!("Failed to parse MIDI packet header: {}", e)))?;

        Ok(Self { header, data: body })
    }

    pub fn commands(&self) -> MidiCommandIterator<'a> {
        MidiCommandIterator::new(self.data)
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

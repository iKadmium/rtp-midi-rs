use super::midi_packet_header::MidiPacketHeaderFlags;

const MIDI_PACKET_HEADER_SIZE: usize = 12;

#[derive(Debug)]
pub struct MidiPacketHeaderZeroAlloc<'a> {
    pub data: &'a [u8; MIDI_PACKET_HEADER_SIZE],
}

impl<'a> MidiPacketHeaderZeroAlloc<'a> {
    pub const fn size() -> usize {
        MIDI_PACKET_HEADER_SIZE
    }

    pub fn new(data: &'a [u8; MIDI_PACKET_HEADER_SIZE]) -> Self {
        Self { data }
    }

    #[allow(dead_code)]
    pub fn flags(&self) -> MidiPacketHeaderFlags {
        MidiPacketHeaderFlags::from(u16::from_be_bytes([self.data[0], self.data[1]]))
    }

    pub fn sequence_number(&self) -> u16 {
        u16::from_be_bytes([self.data[2], self.data[3]])
    }

    #[allow(dead_code)]
    pub fn timestamp(&self) -> u32 {
        u32::from_be_bytes([self.data[4], self.data[5], self.data[6], self.data[7]])
    }

    #[allow(dead_code)]
    pub fn ssrc(&self) -> u32 {
        u32::from_be_bytes([self.data[8], self.data[9], self.data[10], self.data[11]])
    }
}

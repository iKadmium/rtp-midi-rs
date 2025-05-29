use super::{midi_command_iterator::MidiCommandIterator, midi_packet_header_zero_alloc::MidiPacketHeaderZeroAlloc};

#[derive(Debug)]
pub(crate) struct MidiPacketZeroAlloc<'a> {
    header: MidiPacketHeaderZeroAlloc<'a>,
    data: &'a [u8],
}

impl<'a> MidiPacketZeroAlloc<'a> {
    pub fn new(data: &'a [u8]) -> std::io::Result<Self> {
        let header = MidiPacketHeaderZeroAlloc::new(
            data.get(..MidiPacketHeaderZeroAlloc::size())
                .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid MIDI packet header length"))?
                .try_into()
                .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid MIDI packet header length"))?,
        );
        Ok(Self { header, data })
    }

    pub fn commands(&self) -> MidiCommandIterator<'a> {
        MidiCommandIterator::new(&self.data[MidiPacketHeaderZeroAlloc::size()..])
    }

    pub fn sequence_number(&self) -> u16 {
        self.header.sequence_number()
    }

    #[allow(dead_code)]
    pub fn timestamp(&self) -> u32 {
        self.header.timestamp()
    }

    #[allow(dead_code)]
    pub fn ssrc(&self) -> u32 {
        self.header.ssrc()
    }
}

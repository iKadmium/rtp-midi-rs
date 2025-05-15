use log::{info, trace};

use super::{
    midi_command::MidiCommand, midi_command_section::MidiCommandSection,
    midi_packet_header::MidiPacketHeader, midi_timed_command::TimedCommand,
};

#[derive(Debug)]
#[allow(dead_code)]
pub struct MidiPacket {
    header: MidiPacketHeader,
    command_section: MidiCommandSection,
    //recovery_journal: Option<RecoveryJournal>,
}

impl MidiPacket {
    pub fn new(sequence_number: u16, timestamp: u32, ssrc: u32) -> Self {
        MidiPacket {
            header: MidiPacketHeader::new(sequence_number, timestamp, ssrc),
            command_section: MidiCommandSection::new(),
            //recovery_journal: None,
        }
    }

    pub fn from_be_bytes(bytes: &[u8]) -> Result<Self, std::io::Error> {
        let header_bytes = &bytes[0..size_of::<MidiPacketHeader>()];
        let header = MidiPacketHeader::from_be_bytes(header_bytes)?;
        trace!("Parsed header: {:#?}", header);

        let command_section_bytes = &bytes[size_of::<MidiPacketHeader>()..];
        let command_section = MidiCommandSection::from_be_bytes(command_section_bytes)?;
        trace!("Parsed command section: {:#?}", command_section);

        // let recovery_journal = if command_section.j_flag() {
        //     let bytes_read = command_section_bytes.len() + command_section.length() as usize;
        //     let journal_bytes = &bytes[bytes_read..];
        //     Some(RecoveryJournal::from_be_bytes(journal_bytes)?)
        // } else {
        //     None
        // };

        Ok(Self {
            header,
            command_section,
            //recovery_journal,
        })
    }

    pub fn write_to_bytes(&self, bytes: &mut [u8]) -> Result<usize, std::io::Error> {
        let mut bytes_written = self.header.write_to_bytes(bytes)?;
        bytes_written += self
            .command_section
            .write_to_bytes(&mut bytes[size_of::<MidiPacketHeader>()..])?;
        Ok(bytes_written)
    }

    pub fn commands(&self) -> &[TimedCommand] {
        self.command_section.commands()
    }
}

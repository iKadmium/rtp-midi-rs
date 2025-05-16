use log::{info, trace};

use super::{
    midi_command_section::MidiCommandSection, midi_packet_header::MidiPacketHeader,
    midi_timed_command::TimedCommand,
};

#[derive(Debug)]
#[allow(dead_code)]
pub struct MidiPacket {
    header: MidiPacketHeader,
    command_section: MidiCommandSection,
    //recovery_journal: Option<RecoveryJournal>,
}

impl MidiPacket {
    pub fn new(sequence_number: u16, timestamp: u32, ssrc: u32, commands: &[TimedCommand]) -> Self {
        MidiPacket {
            header: MidiPacketHeader::new(sequence_number, timestamp, ssrc),
            command_section: MidiCommandSection::new(commands),
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

    pub fn size(&self) -> usize {
        size_of::<MidiPacketHeader>() + self.command_section.size()
    }

    pub fn commands(&self) -> &[TimedCommand] {
        self.command_section.commands()
    }

    pub fn sequence_number(&self) -> u16 {
        self.header.sequence_number()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packet::midi_packet::midi_command::MidiCommand;
    use crate::packet::midi_packet::midi_timed_command::TimedCommand;

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

        let packet = MidiPacket::new(sequence_number, timestamp, ssrc, &[timed_command]);

        let mut bytes = vec![0; packet.size()];
        packet.write_to_bytes(&mut bytes).unwrap();

        let parsed_packet = MidiPacket::from_be_bytes(&bytes).unwrap();

        assert_eq!(
            packet.header.sequence_number(),
            parsed_packet.header.sequence_number()
        );
        assert_eq!(packet.header.timestamp(), parsed_packet.header.timestamp());
        assert_eq!(packet.header.ssrc(), parsed_packet.header.ssrc());
        assert_eq!(
            packet.command_section.commands().len(),
            parsed_packet.command_section.commands().len()
        );
        assert_eq!(packet.command_section, parsed_packet.command_section);
    }
}

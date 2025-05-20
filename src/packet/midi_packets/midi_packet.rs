use std::io::{Cursor, Write};

use log::trace;

use super::{
    midi_command_list_body::MidiCommandListBody,
    midi_command_list_header::{MidiCommandListFlags, MidiCommandListHeader},
    midi_packet_header::MidiPacketHeader,
    midi_timed_command::TimedCommand,
};

#[derive(Debug)]
#[allow(dead_code)]
pub struct MidiPacket {
    header: MidiPacketHeader,
    command_list: MidiCommandListBody,
    //recovery_journal: Option<RecoveryJournal>,
}

impl MidiPacket {
    pub(crate) fn new(sequence_number: u16, timestamp: u32, ssrc: u32, commands: &[TimedCommand]) -> Self {
        MidiPacket {
            header: MidiPacketHeader::new(sequence_number, timestamp, ssrc),
            command_list: MidiCommandListBody::new(commands),
            //recovery_journal: None,
        }
    }

    pub(in crate::packet) fn from_be_bytes(bytes: &[u8]) -> Result<Self, std::io::Error> {
        let mut reader = Cursor::new(bytes);

        let header = MidiPacketHeader::read(&mut reader)?;
        trace!("Parsed header: {:#?}", header);

        let command_section_header = MidiCommandListHeader::read(&mut reader)?;

        let command_section_start = reader.position() as usize;
        let command_section_end = command_section_start + command_section_header.length() as usize;
        let mut command_section_cursor = Cursor::new(&bytes[command_section_start..command_section_end]);

        let command_section = MidiCommandListBody::read(&mut command_section_cursor, command_section_header.flags().z_flag())?;
        trace!("Parsed command section: {:#?}", command_section);

        // let recovery_journal: Option<u8> = if command_section_header.flags().j_flag() {
        //     let journal_bytes = &bytes[i..];
        //     //Some(RecoveryJournal::from_be_bytes(journal_bytes)?)
        //     None // Placeholder for recovery journal parsing
        // } else {
        //     None
        // };

        Ok(Self {
            header,
            command_list: command_section,
            //recovery_journal,
        })
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
        return size_of::<MidiPacketHeader>() + command_section_header_size + command_section_size;
    }

    pub fn commands(&self) -> &[TimedCommand] {
        self.command_list.commands()
    }

    pub fn sequence_number(&self) -> u16 {
        self.header.sequence_number()
    }

    pub fn timestamp(&self) -> u32 {
        self.header.timestamp()
    }

    pub fn ssrc(&self) -> u32 {
        self.header.ssrc()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packet::midi_packets::midi_command::MidiCommand;
    use crate::packet::midi_packets::midi_timed_command::TimedCommand;

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

        let mut bytes = vec![0; packet.size(false)];
        packet.write(&mut Cursor::new(&mut bytes), false).unwrap();

        let parsed_packet = MidiPacket::from_be_bytes(&bytes).unwrap();

        assert_eq!(packet.header.sequence_number(), parsed_packet.header.sequence_number());
        assert_eq!(packet.header.timestamp(), parsed_packet.header.timestamp());
        assert_eq!(packet.header.ssrc(), parsed_packet.header.ssrc());
        assert_eq!(packet.command_list.commands().len(), parsed_packet.command_list.commands().len());
        assert_eq!(packet.command_list, parsed_packet.command_list);
    }
}

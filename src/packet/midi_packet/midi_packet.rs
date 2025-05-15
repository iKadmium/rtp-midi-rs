use std::io::Cursor;

use bitstream_io::{BigEndian, BitRead, BitReader, BitWrite, FromBitStream, ToBitStream};
use log::trace;

use crate::packet::midi_packet::recovery_journal::recovery_journal::RecoveryJournal;

use super::{midi_command::MidiCommand, midi_command_section::MidiCommandSection};

#[derive(Debug)]
#[allow(dead_code)]
pub struct MidiPacket {
    pub sender_ssrc: u32,
    pub timestamp: u32,
    pub sequence_number: u16,
    pub commands: Vec<MidiCommand>,
    pub recovery_journal: Option<RecoveryJournal>,
}

impl MidiPacket {
    pub fn parse(bytes: &[u8]) -> Result<Self, std::io::Error> {
        let mut reader = BitReader::endian(Cursor::new(bytes), BigEndian);
        let header = reader.parse::<MidiPacketHeader>()?;
        trace!("Parsed header: {:?}", header);
        let command_section = reader.parse::<MidiCommandSection>()?;
        let recovery_journal = if command_section.has_journal {
            Some(reader.parse::<RecoveryJournal>()?)
        } else {
            None
        };

        Ok(Self {
            sequence_number: header.sequence_number,
            sender_ssrc: header.ssrc,
            timestamp: header.timestamp,
            commands: command_section.commands,
            recovery_journal,
        })
    }
}

impl ToBitStream for MidiPacket {
    type Error = std::io::Error;

    fn to_writer<W: BitWrite + ?Sized>(&self, writer: &mut W) -> Result<(), Self::Error> {
        let header = MidiPacketHeader {
            version: 2,
            p_flag: false,
            x_flag: false,
            cc: 0,
            m_flag: true,
            pt: 0x61,
            sequence_number: 0,
            timestamp: self.timestamp,
            ssrc: self.sender_ssrc,
        };
        header.to_writer(writer)?;

        let command_section = MidiCommandSection {
            phantom_flag: false,
            has_journal: false,
            commands: self.commands.clone(),
        };
        command_section.to_writer(writer)?;

        Ok(())
    }
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct MidiPacketHeader {
    pub version: u8,          // Version (should be 2)
    pub p_flag: bool,         // P flag (should be 0)
    pub x_flag: bool,         // X flag (should be 0)
    pub cc: u8,               // CC field (should be 0)
    pub m_flag: bool,         // M flag (should be 1)
    pub pt: u8,               // PT field (should be 0x61)
    pub sequence_number: u16, // Sequence number
    pub timestamp: u32,       // Lower 32 bits of the timestamp in 100-microsecond units
    pub ssrc: u32,            // Sender SSRC
}

impl FromBitStream for MidiPacketHeader {
    type Error = std::io::Error;

    fn from_reader<R: BitRead + ?Sized>(reader: &mut R) -> Result<Self, Self::Error> {
        let version = reader.read::<2, u8>()?;
        let p_flag = reader.read_bit()?;
        let x_flag = reader.read_bit()?;
        let cc = reader.read::<4, u8>()?;
        let m_flag = reader.read_bit()?;
        let pt = reader.read::<7, u8>()?;
        let sequence_number = reader.read::<16, u16>()?;
        let timestamp = reader.read::<32, u32>()?;
        let ssrc = reader.read::<32, u32>()?;

        Ok(Self {
            version,
            p_flag,
            x_flag,
            cc,
            m_flag,
            pt,
            sequence_number,
            timestamp,
            ssrc,
        })
    }
}

impl ToBitStream for MidiPacketHeader {
    type Error = std::io::Error;

    fn to_writer<W: BitWrite + ?Sized>(&self, writer: &mut W) -> std::io::Result<()> {
        writer.write::<2, _>(self.version)?;
        writer.write_bit(self.p_flag)?;
        writer.write_bit(self.x_flag)?;
        writer.write::<4, _>(self.cc)?;
        writer.write_bit(self.m_flag)?;
        writer.write::<7, _>(self.pt)?;
        writer.write::<16, _>(self.sequence_number)?;
        writer.write::<32, _>(self.timestamp)?;
        writer.write::<32, _>(self.ssrc)?;

        Ok(())
    }
}

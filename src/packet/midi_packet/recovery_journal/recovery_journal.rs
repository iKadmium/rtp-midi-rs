use std::{collections::HashMap, fmt::Debug};

use bitstream_io::FromBitStream;

use super::{
    control_change_chapter::ControlChangeChapter, program_change_chapter::ProgramChangeChapter,
};

#[derive(Debug)]
#[allow(dead_code)]
pub struct RecoveryJournal {
    pub s_flag: bool,
    pub a_flag: bool,
    pub h_flag: bool,
    pub total_channels: u8,
    pub checkpoint_sequence_number: u32,
    pub system_journal: Option<SystemJournal>, // Optional system journal
    pub channel_journals: std::collections::HashMap<u8, ChannelJournal>, // Dictionary of channel journals
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct SystemJournal {
    pub s_flag: bool,
    pub d_flag: bool,
    pub v_flag: bool,
    pub q_flag: bool,
    pub f_flag: bool,
    pub x_flag: bool,
    pub system_chapters: Vec<u8>, // Variable-length system chapters
}

#[allow(dead_code)]
enum SystemJournalType {
    S,
    D,
    V,
    Q,
    F,
    X,
}

#[derive(Debug, Hash, Eq, PartialEq)]
#[allow(dead_code)]
pub enum ChannelJournalType {
    ProgramChange,
    ControlChange,
    ParameterSystem,
    PitchWheel,
    NoteOffOn,
    NoteCommandExtras,
    ChannelAftertouch,
    PolyAftertouch,
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct ChannelJournal {
    pub s_flag: bool,                                                 // S flag
    pub channel: u8,                                                  // Channel number (1-4 bits)
    pub h_flag: bool,                                                 // H flag
    pub chapters: HashMap<ChannelJournalType, ChannelJournalChapter>, // Collection of chapters
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum ChannelJournalChapter {
    ProgramChange(ProgramChangeChapter),
    ControlChange(ControlChangeChapter),
    ParameterSystem,
    PitchWheel,
    NoteOffOn,
    NoteCommandExtras,
    ChannelAftertouch,
    PolyAftertouch,
}

impl FromBitStream for RecoveryJournal {
    type Error = std::io::Error;

    fn from_reader<R: bitstream_io::BitRead + ?Sized>(reader: &mut R) -> Result<Self, Self::Error> {
        let s_flag = reader.read_bit()?;
        let a_flag = reader.read_bit()?;
        let h_flag = reader.read_bit()?;
        let total_channels = reader.read::<4, u8>()?;
        let checkpoint_sequence_number = reader.read::<16, u32>()?;

        // Parse system journal if y_flag is set
        let system_journal = if reader.read_bit()? {
            let s_flag = reader.read_bit()?;
            let d_flag = reader.read_bit()?;
            let v_flag = reader.read_bit()?;
            let q_flag = reader.read_bit()?;
            let f_flag = reader.read_bit()?;
            let x_flag = reader.read_bit()?;
            let length = reader.read::<8, u8>()?;

            // Read system chapters
            let mut system_chapters = Vec::new();
            for _ in 0..length {
                system_chapters.push(reader.read::<8, u8>()?);
            }

            Some(SystemJournal {
                s_flag,
                d_flag,
                v_flag,
                q_flag,
                f_flag,
                x_flag,
                system_chapters,
            })
        } else {
            None
        };

        // Parse channel journals
        let mut channel_journals = HashMap::new();
        for _ in 0..total_channels {
            let s_flag = reader.read_bit()?;
            let channel = reader.read::<4, u8>()?;
            let h_flag = reader.read_bit()?;
            let _length = reader.read::<10, u16>()?;

            // Read TOC
            let has_program_change_chapter = reader.read_bit()?;
            let has_control_change_chapter = reader.read_bit()?;
            let _has_parameter_system_chapter = reader.read_bit()?;
            let _has_pitch_wheel_chapter = reader.read_bit()?;
            let _has_note_off_on_chapter = reader.read_bit()?;
            let _has_note_command_extras_chapter = reader.read_bit()?;
            let _has_channel_aftertouch_chapter = reader.read_bit()?;
            let _has_poly_aftertouch_chapter = reader.read_bit()?;

            let mut channel_journal = ChannelJournal {
                s_flag,
                channel,
                h_flag,
                chapters: HashMap::new(),
            };

            if has_program_change_chapter {
                let chapter = reader.parse::<ProgramChangeChapter>()?;
                channel_journal.chapters.insert(
                    ChannelJournalType::ProgramChange,
                    ChannelJournalChapter::ProgramChange(chapter),
                );
            }
            if has_control_change_chapter {
                let chapter = reader.parse::<ControlChangeChapter>()?;
                channel_journal.chapters.insert(
                    ChannelJournalType::ControlChange,
                    ChannelJournalChapter::ControlChange(chapter),
                );
            }
            channel_journals.insert(channel, channel_journal);
        }
        Ok(RecoveryJournal {
            s_flag,
            a_flag,
            h_flag,
            total_channels,
            checkpoint_sequence_number,
            system_journal,
            channel_journals,
        })
    }
}

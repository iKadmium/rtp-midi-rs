use std::{collections::HashMap, fmt::Debug};

use super::{
    control_change_chapter::ControlChangeChapter, program_change_chapter::ProgramChangeChapter,
};

#[derive(Debug)]
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
pub struct SystemJournal {
    pub s_flag: bool,
    pub d_flag: bool,
    pub v_flag: bool,
    pub q_flag: bool,
    pub f_flag: bool,
    pub x_flag: bool,
    pub system_chapters: Vec<u8>, // Variable-length system chapters
}

enum SystemJournalType {
    S,
    D,
    V,
    Q,
    F,
    X,
}

#[derive(Debug, Hash, Eq, PartialEq)]
enum ChannelJournalType {
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
pub struct ChannelJournal {
    pub s_flag: bool,                                            // S flag
    pub channel: u8,                                             // Channel number (1-4 bits)
    pub h_flag: bool,                                            // H flag
    pub length: u16,                                             // Length (10 bits)
    pub chapters: HashMap<ChannelJournalType, Box<dyn Chapter>>, // Collection of chapters
}

pub trait Chapter: Debug {
    fn parse(data: &[u8]) -> Option<(Self, usize)>
    where
        Self: Sized;
}

impl RecoveryJournal {
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        let mut i: usize = 0;
        if bytes.len() < i + 3 {
            return None;
        }

        let journal_header = u32::from_be_bytes([0, bytes[i], bytes[i + 1], bytes[i + 2]]);
        i += 3;

        let s_flag = (journal_header & 0b1000_0000_0000_0000_0000_0000) != 0;
        let y_flag = (journal_header & 0b0100_0000_0000_0000_0000_0000) != 0;
        let a_flag = (journal_header & 0b0010_0000_0000_0000_0000_0000) != 0;
        let h_flag = (journal_header & 0b0001_0000_0000_0000_0000_0000) != 0;
        let total_channels = ((journal_header >> 16) & 0b0000_1111) as u8;
        let checkpoint_sequence_number = journal_header & 0xFFFF;

        let system_journal = if y_flag {
            // Parse system journal if y_flag is set
            if bytes.len() < i + 2 {
                return None; // Not enough data for system journal header
            }

            let system_journal_header = u16::from_be_bytes([bytes[i], bytes[i + 1]]);
            i += 2;

            let s_flag = (system_journal_header & 0b1000_0000_0000_0000) != 0;
            let d_flag = (system_journal_header & 0b0100_0000_0000_0000) != 0;
            let v_flag = (system_journal_header & 0b0010_0000_0000_0000) != 0;
            let q_flag = (system_journal_header & 0b0001_0000_0000_0000) != 0;
            let f_flag = (system_journal_header & 0b0000_1000_0000_0000) != 0;
            let x_flag = (system_journal_header & 0b0000_0100_0000_0000) != 0;
            let length = (system_journal_header & 0b0000_0011_1111_1111) as usize;

            if bytes.len() < i + length {
                return None; // Not enough data for system journal chapters
            }

            let system_chapters = bytes[i..i + length].to_vec();
            i += length;

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

        let mut channel_journals = std::collections::HashMap::new();

        for channel_index in 0..total_channels {
            if bytes.len() < i + 4 {
                return None; // Not enough data for channel journal header
            }

            let channel_journal_header = u16::from_be_bytes([bytes[i], bytes[i + 1]]);

            i += 2;

            let s_flag = (channel_journal_header & 0b1000_0000_0000_0000) != 0;
            let channel: u8 = ((channel_journal_header & 0b0111_1000) >> 8) as u8;
            let h_flag = (channel_journal_header & 0b0000_0100) != 0;
            let length = (channel_journal_header & 0b0000_0011_1111) as u16;

            let toc = bytes[i];

            i += 1;

            let has_program_change_chapter = (toc & 0b1000_0000) != 0;
            let has_control_change_chapter = (toc & 0b0100_0000) != 0;
            let has_parameter_system_chapter = (toc & 0b0010_0000) != 0;
            let has_pitch_wheel_chapter = (toc & 0b0001_0000) != 0;
            let has_note_off_on_chapter = (toc & 0b0000_1000) != 0;
            let has_note_command_extras_chapter = (toc & 0b0000_0100) != 0;
            let has_channel_aftertouch_chapter = (toc & 0b0000_0010) != 0;
            let has_poly_aftertouch_chapter = (toc & 0b0000_0001) != 0;

            let mut channel_journal = ChannelJournal {
                s_flag,
                channel,
                h_flag,
                length,
                chapters: HashMap::new(),
            };

            if bytes.len() < i + length as usize {
                return None; // Not enough data for channel journal chapters
            }

            if has_program_change_chapter {
                let (chapter, chapter_length) = ProgramChangeChapter::parse(&bytes[i..])?;
                channel_journal
                    .chapters
                    .insert(ChannelJournalType::ProgramChange, Box::new(chapter));
                i += chapter_length as usize;
            }

            if has_control_change_chapter {
                let (chapter, chapter_length) = ControlChangeChapter::parse(&bytes[i..])?;
                channel_journal
                    .chapters
                    .insert(ChannelJournalType::ControlChange, Box::new(chapter));
                i += chapter_length as usize;
            }

            channel_journals.insert(channel, channel_journal);
        }

        Some(RecoveryJournal {
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

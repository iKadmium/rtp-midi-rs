use bytes::{BufMut, BytesMut};

use crate::packets::midi_packets::util::StatusBit;

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
#[repr(u8)]
pub enum MidiCommand<'a> {
    NoteOn { channel: u8, key: u8, velocity: u8 } = 0x90,
    NoteOff { channel: u8, key: u8, velocity: u8 } = 0x80,
    PolyphonicKeyPressure { channel: u8, key: u8, pressure: u8 } = 0xA0,
    ControlChange { channel: u8, controller: u8, value: u8 } = 0xB0,
    ProgramChange { channel: u8, program: u8 } = 0xC0,
    ChannelPressure { channel: u8, pressure: u8 } = 0xD0,
    PitchBend { channel: u8, lsb: u8, msb: u8 } = 0xE0,
    SysEx { data: &'a [u8] } = 0xF0, // System Exclusive message
}

impl MidiCommand<'_> {
    pub(crate) fn size(&self) -> usize {
        match self {
            MidiCommand::SysEx { data } => data.len() + 1,
            MidiCommand::NoteOff { .. } => 2,
            MidiCommand::NoteOn { .. } => 2,
            MidiCommand::PolyphonicKeyPressure { .. } => 2,
            MidiCommand::ControlChange { .. } => 2,
            MidiCommand::ProgramChange { .. } => 1,
            MidiCommand::ChannelPressure { .. } => 1,
            MidiCommand::PitchBend { .. } => 2,
        }
    }

    pub(crate) fn status(&self) -> u8 {
        match self {
            MidiCommand::SysEx { .. } => 0xF0,
            MidiCommand::NoteOff { channel, .. } => 0x80 | (channel & 0x0F),
            MidiCommand::NoteOn { channel, .. } => 0x90 | (channel & 0x0F),
            MidiCommand::PolyphonicKeyPressure { channel, .. } => 0xA0 | (channel & 0x0F),
            MidiCommand::ControlChange { channel, .. } => 0xB0 | (channel & 0x0F),
            MidiCommand::ProgramChange { channel, .. } => 0xC0 | (channel & 0x0F),
            MidiCommand::ChannelPressure { channel, .. } => 0xD0 | (channel & 0x0F),
            MidiCommand::PitchBend { channel, .. } => 0xE0 | (channel & 0x0F),
        }
    }

    pub fn to_owned(&self) -> MidiCommand<'static> {
        match self {
            MidiCommand::SysEx { data } => {
                let owned: Vec<u8> = data.to_vec();
                MidiCommand::SysEx {
                    data: Box::leak(owned.into_boxed_slice()),
                }
            }
            MidiCommand::NoteOff { channel, key, velocity } => MidiCommand::NoteOff {
                channel: *channel,
                key: *key,
                velocity: *velocity,
            },
            MidiCommand::NoteOn { channel, key, velocity } => MidiCommand::NoteOn {
                channel: *channel,
                key: *key,
                velocity: *velocity,
            },
            MidiCommand::PolyphonicKeyPressure { channel, key, pressure } => MidiCommand::PolyphonicKeyPressure {
                channel: *channel,
                key: *key,
                pressure: *pressure,
            },
            MidiCommand::ControlChange { channel, controller, value } => MidiCommand::ControlChange {
                channel: *channel,
                controller: *controller,
                value: *value,
            },
            MidiCommand::ProgramChange { channel, program } => MidiCommand::ProgramChange {
                channel: *channel,
                program: *program,
            },
            MidiCommand::ChannelPressure { channel, pressure } => MidiCommand::ChannelPressure {
                channel: *channel,
                pressure: *pressure,
            },
            MidiCommand::PitchBend { channel, lsb, msb } => MidiCommand::PitchBend {
                channel: *channel,
                lsb: *lsb,
                msb: *msb,
            },
        }
    }

    pub(super) fn write(&self, writer: &mut BytesMut, running_status: Option<u8>) {
        if running_status.is_none() || self.status() != running_status.unwrap() {
            writer.put_u8(self.status());
        }

        match self {
            MidiCommand::SysEx { data } => {
                writer.put_u8(0xF0);
                writer.put_slice(data);
                writer.put_u8(0xF7);
            }
            MidiCommand::NoteOff { key, velocity, .. } | MidiCommand::NoteOn { key, velocity, .. } => {
                writer.put_u8(*key);
                writer.put_u8(*velocity);
            }
            MidiCommand::PolyphonicKeyPressure { key, pressure, .. } => {
                writer.put_u8(*key);
                writer.put_u8(*pressure);
            }
            MidiCommand::ControlChange { controller, value, .. } => {
                writer.put_u8(*controller);
                writer.put_u8(*value);
            }
            MidiCommand::ProgramChange { program, .. } => {
                writer.put_u8(*program);
            }
            MidiCommand::ChannelPressure { pressure, .. } => {
                writer.put_u8(*pressure);
            }
            MidiCommand::PitchBend { lsb, msb, .. } => {
                writer.put_u8(*lsb);
                writer.put_u8(*msb);
            }
        }
    }

    fn from_status_byte(status_byte: u8, channel: u8, bytes: &[u8]) -> (MidiCommand<'_>, &[u8]) {
        let command = match status_byte {
            0x80 => MidiCommand::NoteOff {
                channel,
                key: bytes[0],
                velocity: bytes[1],
            },
            0x90 => MidiCommand::NoteOn {
                channel,
                key: bytes[0],
                velocity: bytes[1],
            },
            0xA0 => MidiCommand::PolyphonicKeyPressure {
                channel,
                key: bytes[0],
                pressure: bytes[1],
            },
            0xB0 => MidiCommand::ControlChange {
                channel,
                controller: bytes[0],
                value: bytes[1],
            },
            0xC0 => MidiCommand::ProgramChange { channel, program: bytes[0] },
            0xD0 => MidiCommand::ChannelPressure { channel, pressure: bytes[0] },
            0xE0 => MidiCommand::PitchBend {
                channel,
                lsb: bytes[0],
                msb: bytes[1],
            },

            0xF0 => {
                todo!("Handle SysEx command");
            }
            _ => panic!("Unknown MIDI command type"),
        };

        let remaining = &bytes[command.size()..];
        (command, remaining)
    }

    pub(crate) fn from_be_bytes(bytes: &[u8], running_status: Option<u8>) -> std::io::Result<(MidiCommand, &[u8])> {
        let (status_byte, bytes) = if bytes[0].status_bit() {
            (bytes[0], &bytes[1..])
        } else {
            (
                running_status.ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "Running status not set"))?,
                bytes,
            )
        };
        let channel = status_byte & 0x0F;
        let command_type = status_byte & 0xF0;
        Ok(MidiCommand::from_status_byte(command_type, channel, bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_midi_command() {
        let command = MidiCommand::NoteOn {
            channel: 7,
            key: 0x40,
            velocity: 0x7F,
        };
        assert_eq!(command.status(), 0x97);
        assert_eq!(command.size(), 2);
        // Check fields
        if let MidiCommand::NoteOn { key, velocity, channel } = command {
            assert_eq!(channel, 7);
            assert_eq!(key, 0x40);
            assert_eq!(velocity, 0x7F);
        } else {
            panic!("Not a NoteOn command");
        }
    }

    #[test]
    fn test_midi_command_write() {
        let command = MidiCommand::NoteOn {
            channel: 4,
            key: 0x40,
            velocity: 0x7F,
        };
        let mut bytes = BytesMut::new();
        command.write(&mut bytes, None);
        assert_eq!(bytes.len(), 3);
        assert_eq!(bytes[..3], [0x94, 0x40, 0x7F]);
    }

    fn test_command_write_type(command: MidiCommand, expected_bytes: &[u8]) {
        let mut bytes = BytesMut::new();
        command.write(&mut bytes, None);
        assert_eq!(bytes.len(), expected_bytes.len());
        assert_eq!(bytes, expected_bytes);
    }

    #[test]
    fn test_command_write_note_off() {
        let command = MidiCommand::NoteOff {
            channel: 4,
            key: 0x40,
            velocity: 0x7F,
        };
        let expected_bytes: Vec<u8> = vec![0x84u8, 0x40, 0x7F];
        test_command_write_type(command, &expected_bytes);
    }

    #[test]
    fn test_command_write_note_on() {
        let command = MidiCommand::NoteOn {
            channel: 4,
            key: 0x40,
            velocity: 0x7F,
        };
        let expected_bytes: Vec<u8> = vec![0x94u8, 0x40, 0x7F];
        test_command_write_type(command, &expected_bytes);
    }

    #[test]
    fn test_command_write_polyphonic_key_pressure() {
        let command = MidiCommand::PolyphonicKeyPressure {
            channel: 4,
            key: 0x40,
            pressure: 0x7F,
        };
        let expected_bytes: Vec<u8> = vec![0xA4u8, 0x40, 0x7F];
        test_command_write_type(command, &expected_bytes);
    }

    #[test]
    fn test_command_write_control_change() {
        let command = MidiCommand::ControlChange {
            channel: 4,
            controller: 0x40,
            value: 0x7F,
        };
        let expected_bytes: Vec<u8> = vec![0xB4u8, 0x40, 0x7F];
        test_command_write_type(command, &expected_bytes);
    }

    #[test]
    fn test_command_write_program_change() {
        let command = MidiCommand::ProgramChange { channel: 4, program: 0x40 };
        let expected_bytes: Vec<u8> = vec![0xC4u8, 0x40];
        test_command_write_type(command, &expected_bytes);
    }

    #[test]
    fn test_command_write_channel_pressure() {
        let command = MidiCommand::ChannelPressure { channel: 4, pressure: 0x40 };
        let expected_bytes: Vec<u8> = vec![0xD4u8, 0x40];
        test_command_write_type(command, &expected_bytes);
    }

    #[test]
    fn test_command_write_pitch_bend() {
        let command = MidiCommand::PitchBend {
            channel: 4,
            lsb: 0x40,
            msb: 0x7F,
        };
        let expected_bytes: Vec<u8> = vec![0xE4u8, 0x40, 0x7F];
        test_command_write_type(command, &expected_bytes);
    }

    #[test]
    fn test_command_write_invalid() {
        let command = MidiCommand::NoteOn {
            channel: 4,
            key: 0x40,
            velocity: 0x7F,
        };
        let mut bytes = BytesMut::new();
        command.write(&mut bytes, None);
        assert_eq!(&bytes[..], &[0x94u8, 0x40, 0x7F]);
    }
}

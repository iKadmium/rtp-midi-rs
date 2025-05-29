use byteorder::ReadBytesExt;
use byteorder::WriteBytesExt;
use std::io::Error;
use std::io::ErrorKind;
use std::io::{Read, Write};

use super::util::StatusBit;

#[derive(Debug, Clone, PartialEq)] // Removed `Copy` trait as `SysEx` uses `Vec<u8>`
#[repr(u8)]
#[allow(dead_code)]
pub enum MidiCommand {
    NoteOff { channel: u8, key: u8, velocity: u8 },
    NoteOn { channel: u8, key: u8, velocity: u8 },
    PolyphonicKeyPressure { channel: u8, key: u8, pressure: u8 },
    ControlChange { channel: u8, controller: u8, value: u8 },
    ProgramChange { channel: u8, program: u8 },
    ChannelPressure { channel: u8, pressure: u8 },
    PitchBend { channel: u8, lsb: u8, msb: u8 },
    SysEx { data: Vec<u8> }, // System Exclusive message
}

impl MidiCommand {
    pub(crate) fn size(&self) -> usize {
        match self {
            MidiCommand::SysEx { data } => data.len() + 2,
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

    fn size_from_status(status: u8) -> usize {
        match status & 0xF0 {
            0x80 => 2, // Note Off
            0x90 => 2, // Note On
            0xA0 => 2, // Polyphonic Key Pressure
            0xB0 => 2, // Control Change
            0xC0 => 1, // Program Change
            0xD0 => 1, // Channel Pressure
            0xE0 => 2, // Pitch Bend
            _ => 0,
        }
    }

    pub(super) fn from_be_bytes(data: &[u8], running_status: Option<u8>) -> Result<(Self, usize), Error> {
        let first_byte = data[0];
        if first_byte == 0xF0 {
            let end = data
                .iter()
                .position(|&b| b == 0xF7)
                .ok_or_else(|| Error::new(ErrorKind::InvalidData, "SysEx message not terminated with 0xF7"))?;
            let sysex_data = &data[1..end];
            return Ok((MidiCommand::SysEx { data: sysex_data.to_vec() }, end + 1));
        }

        let (status, data_bytes_read) = if first_byte.status_bit() {
            (first_byte, 1)
        } else {
            match running_status {
                Some(rs) => (rs, 0),
                None => {
                    return Err(Error::new(ErrorKind::InvalidData, "No status with no running status byte"));
                }
            }
        };
        let channel = status & 0x0F;
        let size = MidiCommand::size_from_status(status);

        let command = match status & 0xF0 {
            0x80 => MidiCommand::NoteOff {
                channel,
                key: data[data_bytes_read],
                velocity: data[data_bytes_read + 1],
            },
            0x90 => MidiCommand::NoteOn {
                channel,
                key: data[data_bytes_read],
                velocity: data[data_bytes_read + 1],
            },
            0xA0 => MidiCommand::PolyphonicKeyPressure {
                channel,
                key: data[data_bytes_read],
                pressure: data[data_bytes_read + 1],
            },
            0xB0 => MidiCommand::ControlChange {
                channel,
                controller: data[data_bytes_read],
                value: data[data_bytes_read + 1],
            },
            0xC0 => MidiCommand::ProgramChange {
                channel,
                program: data[data_bytes_read],
            },
            0xD0 => MidiCommand::ChannelPressure {
                channel,
                pressure: data[data_bytes_read],
            },
            0xE0 => MidiCommand::PitchBend {
                channel,
                lsb: data[data_bytes_read],
                msb: data[data_bytes_read + 1],
            },
            _ => {
                return Err(Error::new(ErrorKind::InvalidData, "Invalid MIDI command"));
            }
        };

        let bytes_read = data_bytes_read + size;

        Ok((command, bytes_read))
    }

    pub(super) fn read<R: Read>(reader: &mut R, running_status: Option<u8>) -> Result<Self, std::io::Error> {
        let first_byte = reader.read_u8()?;
        if first_byte == 0xF0 {
            let mut data = Vec::new();
            loop {
                let byte = reader.read_u8()?;
                if byte == 0xF7 {
                    break;
                }
                data.push(byte);
            }
            return Ok(MidiCommand::SysEx { data });
        }
        let mut data: [u8; 2] = [0; 2];

        let (status, data_bytes_read) = if first_byte & 0x80 == 0 {
            match running_status {
                Some(rs) => {
                    data[0] = first_byte;
                    (rs, 1)
                }
                None => {
                    return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "No status with no running status byte"));
                }
            }
        } else {
            (first_byte, 0)
        };
        let channel = status & 0x0F;
        let size = MidiCommand::size_from_status(status);

        for byte in data[data_bytes_read..size].iter_mut() {
            *byte = reader.read_u8()?;
        }

        let command = match status & 0xF0 {
            0x80 => MidiCommand::NoteOff {
                channel,
                key: data[0],
                velocity: data[1],
            },
            0x90 => MidiCommand::NoteOn {
                channel,
                key: data[0],
                velocity: data[1],
            },
            0xA0 => MidiCommand::PolyphonicKeyPressure {
                channel,
                key: data[0],
                pressure: data[1],
            },
            0xB0 => MidiCommand::ControlChange {
                channel,
                controller: data[0],
                value: data[1],
            },
            0xC0 => MidiCommand::ProgramChange { channel, program: data[0] },
            0xD0 => MidiCommand::ChannelPressure { channel, pressure: data[0] },
            0xE0 => MidiCommand::PitchBend {
                channel,
                lsb: data[0],
                msb: data[1],
            },
            _ => {
                return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid MIDI command"));
            }
        };

        Ok(command)
    }

    pub(super) fn write<W: Write>(&self, writer: &mut W, running_status: Option<u8>) -> Result<usize, std::io::Error> {
        let mut bytes_written = 0;
        if running_status.is_none() || self.status() != running_status.unwrap() {
            writer.write_u8(self.status())?;
            bytes_written += 1;
        }

        match self {
            MidiCommand::SysEx { data } => {
                writer.write_u8(0xF0)?;
                bytes_written += 1;
                writer.write_all(data)?;
                bytes_written += data.len();
                writer.write_u8(0xF7)?;
                bytes_written += 1;
            }
            MidiCommand::NoteOff { key, velocity, .. } | MidiCommand::NoteOn { key, velocity, .. } => {
                writer.write_u8(*key)?;
                writer.write_u8(*velocity)?;
                bytes_written += 2;
            }
            MidiCommand::PolyphonicKeyPressure { key, pressure, .. } => {
                writer.write_u8(*key)?;
                writer.write_u8(*pressure)?;
                bytes_written += 2;
            }
            MidiCommand::ControlChange { controller, value, .. } => {
                writer.write_u8(*controller)?;
                writer.write_u8(*value)?;
                bytes_written += 2;
            }
            MidiCommand::ProgramChange { program, .. } => {
                writer.write_u8(*program)?;
                bytes_written += 1;
            }
            MidiCommand::ChannelPressure { pressure, .. } => {
                writer.write_u8(*pressure)?;
                bytes_written += 1;
            }
            MidiCommand::PitchBend { lsb, msb, .. } => {
                writer.write_u8(*lsb)?;
                writer.write_u8(*msb)?;
                bytes_written += 2;
            }
        }
        Ok(bytes_written)
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

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
    fn test_midi_command_read_with_status_byte() {
        let bytes: Vec<u8> = vec![0x94u8, 0x40, 0x7F, 0x00];
        let mut reader = Cursor::new(bytes);
        let command = MidiCommand::read(&mut reader, None).unwrap();
        assert_eq!(command.status(), 0x94);
        if let MidiCommand::NoteOn { key, velocity, .. } = command {
            assert_eq!(key, 0x40);
            assert_eq!(velocity, 0x7F);
        } else {
            panic!("Not a NoteOn command");
        }
    }

    #[test]
    fn test_midi_command_from_bytes_without_status_byte() {
        let bytes = vec![0x40u8, 0x7F, 0x00];
        let mut reader = Cursor::new(bytes);
        let command = MidiCommand::read(&mut reader, Some(0x94)).unwrap();
        assert_eq!(command.status(), 0x94);
        if let MidiCommand::NoteOn { key, velocity, .. } = command {
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
        let mut bytes = Vec::new();
        let bytes_written = command.write(&mut bytes, None).unwrap();
        assert_eq!(bytes_written, 3);
        assert_eq!(bytes[..3], [0x94, 0x40, 0x7F]);
    }

    #[test]
    fn test_serialize_and_deserialize() {
        let original_command = MidiCommand::NoteOn {
            channel: 4,
            key: 0x40,
            velocity: 0x7F,
        };
        let mut bytes = Vec::new();
        let _ = original_command.write(&mut bytes, None).unwrap();
        let mut reader = Cursor::new(&bytes);
        let deserialized_command = MidiCommand::read(&mut reader, None).unwrap();
        assert_eq!(original_command, deserialized_command);
    }

    fn test_command_read_type(bytes: &[u8], expected_command: MidiCommand) {
        let mut reader = Cursor::new(bytes);
        let command = MidiCommand::read(&mut reader, None).unwrap();
        assert_eq!(command, expected_command);
    }

    #[test]
    fn test_command_read_polyphonic_key_pressure() {
        let bytes: Vec<u8> = vec![0xA4u8, 0x40, 0x7F];
        let expected_command = MidiCommand::PolyphonicKeyPressure {
            channel: 4,
            key: 0x40,
            pressure: 0x7F,
        };
        test_command_read_type(&bytes, expected_command);
    }

    #[test]
    fn test_command_read_control_change() {
        let bytes: Vec<u8> = vec![0xB4u8, 0x40, 0x7F];
        let expected_command = MidiCommand::ControlChange {
            channel: 4,
            controller: 0x40,
            value: 0x7F,
        };
        test_command_read_type(&bytes, expected_command);
    }

    #[test]
    fn test_command_read_program_change() {
        let bytes: Vec<u8> = vec![0xC4u8, 0x40];
        let expected_command = MidiCommand::ProgramChange { channel: 4, program: 0x40 };
        test_command_read_type(&bytes, expected_command);
    }

    #[test]
    fn test_command_read_channel_pressure() {
        let bytes: Vec<u8> = vec![0xD4u8, 0x40];
        let expected_command = MidiCommand::ChannelPressure { channel: 4, pressure: 0x40 };
        test_command_read_type(&bytes, expected_command);
    }

    #[test]
    fn test_command_read_pitch_bend() {
        let bytes: Vec<u8> = vec![0xE4u8, 0x40, 0x7F];
        let expected_command = MidiCommand::PitchBend {
            channel: 4,
            lsb: 0x40,
            msb: 0x7F,
        };
        test_command_read_type(&bytes, expected_command);
    }

    #[test]
    fn test_command_read_invalid() {
        let bytes: Vec<u8> = vec![0xFFu8, 0x40, 0x7F];
        let mut reader = Cursor::new(bytes);
        let result = MidiCommand::read(&mut reader, None);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::InvalidData);
    }

    fn test_command_write_type(command: MidiCommand, expected_bytes: &[u8]) {
        let mut bytes = Vec::new();
        let bytes_written = command.write(&mut bytes, None).unwrap();
        assert_eq!(bytes_written, expected_bytes.len());
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
        let mut bytes = Vec::new();
        let result = command.write(&mut bytes, None);
        assert!(result.is_ok());
        assert_eq!(bytes, vec![0x94u8, 0x40, 0x7F]);
    }

    #[test]
    fn test_command_read_without_running_status() {
        let bytes: Vec<u8> = vec![0x40, 0x7F];
        let mut reader = Cursor::new(bytes);
        let result = MidiCommand::read(&mut reader, None);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::InvalidData);
    }
}

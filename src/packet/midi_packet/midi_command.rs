#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
#[allow(dead_code)]
pub enum MidiCommand {
    NoteOff {
        channel: u8,
        key: u8,
        velocity: u8,
    },
    NoteOn {
        channel: u8,
        key: u8,
        velocity: u8,
    },
    PolyphonicKeyPressure {
        channel: u8,
        key: u8,
        pressure: u8,
    },
    ControlChange {
        channel: u8,
        controller: u8,
        value: u8,
    },
    ProgramChange {
        channel: u8,
        program: u8,
    },
    ChannelPressure {
        channel: u8,
        pressure: u8,
    },
    PitchBend {
        channel: u8,
        lsb: u8,
        msb: u8,
    },
}

impl MidiCommand {
    pub fn size(&self) -> usize {
        match self {
            MidiCommand::NoteOff { .. } => 2,
            MidiCommand::NoteOn { .. } => 2,
            MidiCommand::PolyphonicKeyPressure { .. } => 2,
            MidiCommand::ControlChange { .. } => 2,
            MidiCommand::ProgramChange { .. } => 1,
            MidiCommand::ChannelPressure { .. } => 1,
            MidiCommand::PitchBend { .. } => 2,
        }
    }
    pub fn channel(&self) -> u8 {
        match self {
            MidiCommand::NoteOff { channel, .. }
            | MidiCommand::NoteOn { channel, .. }
            | MidiCommand::PolyphonicKeyPressure { channel, .. }
            | MidiCommand::ControlChange { channel, .. }
            | MidiCommand::ProgramChange { channel, .. }
            | MidiCommand::ChannelPressure { channel, .. }
            | MidiCommand::PitchBend { channel, .. } => *channel,
        }
    }
    pub fn status(&self) -> u8 {
        match self {
            MidiCommand::NoteOff { channel, .. } => 0x80 | (channel & 0x0F),
            MidiCommand::NoteOn { channel, .. } => 0x90 | (channel & 0x0F),
            MidiCommand::PolyphonicKeyPressure { channel, .. } => 0xA0 | (channel & 0x0F),
            MidiCommand::ControlChange { channel, .. } => 0xB0 | (channel & 0x0F),
            MidiCommand::ProgramChange { channel, .. } => 0xC0 | (channel & 0x0F),
            MidiCommand::ChannelPressure { channel, .. } => 0xD0 | (channel & 0x0F),
            MidiCommand::PitchBend { channel, .. } => 0xE0 | (channel & 0x0F),
        }
    }

    pub fn from_be_bytes(
        bytes: &[u8],
        running_status: Option<u8>,
    ) -> Result<(Self, usize), std::io::Error> {
        let mut bytes_read = 0;
        let status = if bytes[bytes_read] & 0x80 == 0 {
            match running_status {
                Some(status) => status,
                None => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "No status bit and running status is not set",
                    ));
                }
            }
        } else {
            bytes_read += 1;
            bytes[bytes_read - 1]
        };
        let channel = status & 0x0F;
        let command_type = status & 0xF0;
        let (cmd, data_len) = match command_type {
            0x80 => {
                let key = bytes[bytes_read];
                let velocity = bytes[bytes_read + 1];
                (
                    MidiCommand::NoteOff {
                        channel,
                        key,
                        velocity,
                    },
                    2,
                )
            }
            0x90 => {
                let key = bytes[bytes_read];
                let velocity = bytes[bytes_read + 1];
                (
                    MidiCommand::NoteOn {
                        channel,
                        key,
                        velocity,
                    },
                    2,
                )
            }
            0xA0 => {
                let key = bytes[bytes_read];
                let pressure = bytes[bytes_read + 1];
                (
                    MidiCommand::PolyphonicKeyPressure {
                        channel,
                        key,
                        pressure,
                    },
                    2,
                )
            }
            0xB0 => {
                let controller = bytes[bytes_read];
                let value = bytes[bytes_read + 1];
                (
                    MidiCommand::ControlChange {
                        channel,
                        controller,
                        value,
                    },
                    2,
                )
            }
            0xC0 => {
                let program = bytes[bytes_read];
                (MidiCommand::ProgramChange { channel, program }, 1)
            }
            0xD0 => {
                let pressure = bytes[bytes_read];
                (MidiCommand::ChannelPressure { channel, pressure }, 1)
            }
            0xE0 => {
                let lsb = bytes[bytes_read];
                let msb = bytes[bytes_read + 1];
                (MidiCommand::PitchBend { channel, lsb, msb }, 2)
            }
            _ => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Invalid MIDI command type",
                ));
            }
        };
        bytes_read += data_len;
        Ok((cmd, bytes_read))
    }

    pub fn write_to_bytes(
        &self,
        bytes: &mut [u8],
        running_status: Option<u8>,
    ) -> Result<usize, std::io::Error> {
        let mut bytes_written = 0;
        let status = self.status();
        let write_status = match running_status {
            Some(rs) => status != rs,
            None => true,
        };
        if write_status {
            bytes[bytes_written] = status;
            bytes_written += 1;
        }
        match self {
            MidiCommand::NoteOff { key, velocity, .. }
            | MidiCommand::NoteOn { key, velocity, .. } => {
                bytes[bytes_written] = *key;
                bytes[bytes_written + 1] = *velocity;
                bytes_written += 2;
            }
            MidiCommand::PolyphonicKeyPressure { key, pressure, .. } => {
                bytes[bytes_written] = *key;
                bytes[bytes_written + 1] = *pressure;
                bytes_written += 2;
            }
            MidiCommand::ControlChange {
                controller, value, ..
            } => {
                bytes[bytes_written] = *controller;
                bytes[bytes_written + 1] = *value;
                bytes_written += 2;
            }
            MidiCommand::ProgramChange { program, .. } => {
                bytes[bytes_written] = *program;
                bytes_written += 1;
            }
            MidiCommand::ChannelPressure { pressure, .. } => {
                bytes[bytes_written] = *pressure;
                bytes_written += 1;
            }
            MidiCommand::PitchBend { lsb, msb, .. } => {
                bytes[bytes_written] = *lsb;
                bytes[bytes_written + 1] = *msb;
                bytes_written += 2;
            }
        }
        Ok(bytes_written)
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
        assert_eq!(command.channel(), 7);
        assert_eq!(command.status(), 0x97);
        assert_eq!(command.size(), 2);
        // Check fields
        if let MidiCommand::NoteOn { key, velocity, .. } = command {
            assert_eq!(key, 0x40);
            assert_eq!(velocity, 0x7F);
        } else {
            panic!("Not a NoteOn command");
        }
    }

    #[test]
    fn test_midi_command_from_bytes_with_status_byte() {
        let bytes: [u8; 4] = [0x94, 0x40, 0x7F, 0x00];
        let (command, bytes_read) = MidiCommand::from_be_bytes(&bytes, None).unwrap();
        assert_eq!(command.status(), 0x94);
        if let MidiCommand::NoteOn { key, velocity, .. } = command {
            assert_eq!(key, 0x40);
            assert_eq!(velocity, 0x7F);
        } else {
            panic!("Not a NoteOn command");
        }
        assert_eq!(bytes_read, 3);
    }

    #[test]
    fn test_midi_command_from_bytes_without_status_byte() {
        let bytes: [u8; 3] = [0x40, 0x7F, 0x00];
        let running_status = Some(0x94);
        let (command, bytes_read) = MidiCommand::from_be_bytes(&bytes, running_status).unwrap();
        assert_eq!(command.status(), 0x94);
        if let MidiCommand::NoteOn { key, velocity, .. } = command {
            assert_eq!(key, 0x40);
            assert_eq!(velocity, 0x7F);
        } else {
            panic!("Not a NoteOn command");
        }
        assert_eq!(bytes_read, 2);
    }

    #[test]
    fn test_midi_command_write_to_bytes() {
        let command = MidiCommand::NoteOn {
            channel: 4,
            key: 0x40,
            velocity: 0x7F,
        };
        let mut bytes = [0u8; 4];
        let bytes_written = command.write_to_bytes(&mut bytes, None).unwrap();
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
        let mut bytes = [0u8; 4];
        let bytes_written = original_command.write_to_bytes(&mut bytes, None).unwrap();
        let (deserialized_command, bytes_read) =
            MidiCommand::from_be_bytes(&bytes[..bytes_written], None).unwrap();
        assert_eq!(original_command, deserialized_command);
        assert_eq!(bytes_written, bytes_read);
    }
}

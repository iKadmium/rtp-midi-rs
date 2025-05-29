use super::{midi_command::MidiCommand, util::StatusBit};

#[derive(Debug)]
pub struct MidiCommandZeroAlloc<'a> {
    status: u8,
    data: &'a [u8],
}

impl<'a> MidiCommandZeroAlloc<'a> {
    pub(crate) fn from_be_bytes(bytes: &'a [u8], running_status: Option<u8>) -> Result<(Self, usize), std::io::Error> {
        let mut offset = 0;
        let status = if bytes[0].status_bit() {
            offset += 1;
            bytes[0]
        } else if let Some(status) = running_status {
            status
        } else {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Running status not set"));
        };

        if status == 0xF0 {
            let slice_end = &bytes[1..]
                .iter()
                .position(|&b| b == 0xF7)
                .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "Incomplete SysEx message"))?;

            let slice = &bytes[1..*slice_end]; // skip the status byte
            return Ok((Self { status, data: slice }, slice_end + 1));
        }

        let length = MidiCommandZeroAlloc::size_from_status(status);
        let data = &bytes[offset..offset + length];

        Ok((Self { status, data }, offset + length))
    }

    pub fn to_owned(&self) -> MidiCommand {
        match self.status & 0xF0 {
            0x80 => MidiCommand::NoteOff {
                channel: self.channel(),
                key: self.data[0],
                velocity: self.data[1],
            },
            0x90 => MidiCommand::NoteOn {
                channel: self.channel(),
                key: self.data[0],
                velocity: self.data[1],
            },
            0xA0 => MidiCommand::PolyphonicKeyPressure {
                channel: self.channel(),
                key: self.data[0],
                pressure: self.data[1],
            },
            0xB0 => MidiCommand::ControlChange {
                channel: self.channel(),
                controller: self.data[0],
                value: self.data[1],
            },
            0xC0 => MidiCommand::ProgramChange {
                channel: self.channel(),
                program: self.data[0],
            },
            0xD0 => MidiCommand::ChannelPressure {
                channel: self.channel(),
                pressure: self.data[0],
            },
            0xE0 => MidiCommand::PitchBend {
                channel: self.channel(),
                lsb: self.data[0],
                msb: self.data[1],
            },
            0xF0 => MidiCommand::SysEx { data: self.data }, // SysEx or other non-standard command
            _ => unreachable!(),
        }
    }

    pub fn channel(&self) -> u8 {
        self.status & 0x0F
    }

    pub fn status(&self) -> u8 {
        self.status
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
}

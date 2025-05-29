use super::{delta_time_zero_alloc::DeltaTimeZeroAlloc, midi_command::MidiCommand, midi_command_zero_alloc::MidiCommandZeroAlloc};

#[derive(Debug)]
pub(crate) struct MidiTimedCommandZeroAlloc<'a> {
    delta_time: Option<DeltaTimeZeroAlloc<'a>>,
    command: MidiCommandZeroAlloc<'a>,
}

impl<'a> MidiTimedCommandZeroAlloc<'a> {
    pub fn from_be_bytes(bytes: &'a [u8], read_delta_time: bool, running_status: Option<u8>) -> std::io::Result<(Self, usize)> {
        let (delta_time, offset) = if read_delta_time {
            let (dt, size) = DeltaTimeZeroAlloc::from_be_bytes(bytes);
            (Some(dt), size)
        } else {
            (None, 0)
        };

        let (command, command_size) = MidiCommandZeroAlloc::from_be_bytes(&bytes[offset..], running_status)?;
        Ok((MidiTimedCommandZeroAlloc { delta_time, command }, offset + command_size))
    }

    pub fn delta_time(&self) -> u32 {
        self.delta_time.as_ref().map_or(0, |dt| dt.delta_time())
    }

    pub fn command(&self) -> MidiCommand {
        self.command.to_owned()
    }
}

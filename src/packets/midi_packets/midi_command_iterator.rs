use super::{midi_command_list_header::MidiCommandListHeader, midi_timed_command_zero_alloc::MidiTimedCommandZeroAlloc};

#[derive(Debug)]
pub(crate) struct MidiCommandIterator<'a> {
    data: &'a [u8],
    offset: usize,
    running_status: Option<u8>,
    read_delta_time: bool,
}

impl<'a> MidiCommandIterator<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        let command_list_header = MidiCommandListHeader::from_slice(data);
        let read_delta_time = command_list_header.flags().z_flag();
        let offset = MidiCommandListHeader::size(command_list_header.flags().b_flag());
        let length = command_list_header.length();
        let slice = &data[offset..length + offset];
        MidiCommandIterator {
            data: slice,
            offset: 0,
            running_status: None,
            read_delta_time,
        }
    }
}

impl<'a> Iterator for MidiCommandIterator<'a> {
    type Item = MidiTimedCommandZeroAlloc<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.offset < self.data.len() {
            match MidiTimedCommandZeroAlloc::from_be_bytes(&self.data[self.offset..], self.read_delta_time, self.running_status) {
                Ok((command, bytes_read)) => {
                    self.running_status = Some(command.command().status());
                    self.offset += bytes_read;
                    self.read_delta_time = true;
                    Some(command)
                }
                Err(_) => None, // Handle error appropriately, e.g., log or return None
            }
        } else {
            None
        }
    }
}

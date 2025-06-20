use crate::packets::midi_packets::midi_event::MidiEvent;

use super::midi_command_list_header::MidiCommandListHeader;

#[derive(Debug)]
pub(crate) struct MidiCommandIterator<'a> {
    data: &'a [u8],
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
            running_status: None,
            read_delta_time,
        }
    }
}

impl<'a> Iterator for MidiCommandIterator<'a> {
    type Item = MidiEvent<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if !self.data.is_empty() {
            match MidiEvent::from_be_bytes(self.data, self.read_delta_time, self.running_status) {
                Ok((command, new_offset)) => {
                    self.running_status = Some(command.command().status());
                    self.data = new_offset;
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

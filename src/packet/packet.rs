use super::{control_packet::ControlPacket, midi_packet::midi_packet::MidiPacket};

#[derive(Debug)]
pub enum RtpMidiPacket {
    Midi(MidiPacket),
    Control(ControlPacket),
}

impl RtpMidiPacket {
    pub fn parse(bytes: &[u8]) -> Result<Self, std::io::Error> {
        if bytes.is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Input data was empty",
            ));
        }

        if ControlPacket::is_control_packet(bytes) {
            return ControlPacket::parse(bytes).map(RtpMidiPacket::Control);
        } else {
            return MidiPacket::parse(bytes).map(RtpMidiPacket::Midi);
        }
    }
}

use super::{control_packets::control_packet::ControlPacket, midi_packets::midi_packet_zero_alloc::MidiPacketZeroAlloc};

#[derive(Debug)]
pub(crate) enum RtpMidiPacket<'a> {
    Midi(MidiPacketZeroAlloc<'a>),
    Control(ControlPacket<'a>),
}

impl<'a> RtpMidiPacket<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self, std::io::Error> {
        if ControlPacket::is_control_packet(bytes) {
            ControlPacket::from_be_bytes(bytes).map(RtpMidiPacket::Control)
        } else {
            MidiPacketZeroAlloc::new(bytes).map(RtpMidiPacket::Midi)
        }
    }
}

#[cfg(test)]
mod tests {
    use zerocopy::U16;
    use zerocopy::network_endian::U32;

    use super::*;
    use crate::packets::midi_packets::midi_command::MidiCommand;
    use crate::packets::midi_packets::midi_packet_builder::MidiPacketBuilder;
    use crate::packets::midi_packets::midi_timed_command::TimedCommand;

    #[test]
    fn test_parse_midi_packet() {
        let commands = vec![TimedCommand::new(
            None,
            MidiCommand::NoteOn {
                channel: 1,
                key: 64,
                velocity: 127,
            },
        )];
        let packet = MidiPacketBuilder::new_as_bytes(U16::new(1), U32::new(2), U32::new(3), &commands);

        let parsed_packet = RtpMidiPacket::parse(&packet).unwrap();
        if let RtpMidiPacket::Midi(parsed_midi_packet) = parsed_packet {
            assert_eq!(parsed_midi_packet.sequence_number(), 1);
            assert_eq!(parsed_midi_packet.timestamp(), 2);
            assert_eq!(parsed_midi_packet.ssrc(), 3);
            let values = parsed_midi_packet.commands().collect::<Vec<_>>();
            assert_eq!(values.len(), 1);
            assert_eq!(
                values[0].command().to_owned(),
                MidiCommand::NoteOn {
                    channel: 1,
                    key: 64,
                    velocity: 127
                }
            );
        } else {
            panic!("Expected MidiPacket");
        }
    }

    #[test]
    fn test_parse_control_packet() {
        let packet = ControlPacket::new_acceptance(U32::new(1), U32::new(1), c"Test Name");
        let parsed = RtpMidiPacket::parse(&packet).unwrap();

        match parsed {
            RtpMidiPacket::Control(ControlPacket::Acceptance { body: _, name: _ }) => {
                // all good
            }
            _ => panic!("Expected ControlPacket"),
        }
    }
}

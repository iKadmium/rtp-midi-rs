use super::{control_packets::control_packet::ControlPacket, midi_packets::midi_packet_zero_alloc::MidiPacketZeroAlloc};

#[derive(Debug)]
pub(crate) enum RtpMidiPacket<'a> {
    Midi(MidiPacketZeroAlloc<'a>),
    Control(ControlPacket),
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
    use super::*;
    use crate::packets::control_packets::session_initiation_packet::SessionInitiationPacket;
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
        let packet = MidiPacketBuilder::new(1, 2, 3, &commands);
        let mut bytes = Vec::new();
        let result = packet.write(&mut bytes, false);
        assert!(result.is_ok());

        let parsed_packet = RtpMidiPacket::parse(&bytes).unwrap();
        if let RtpMidiPacket::Midi(parsed_midi_packet) = parsed_packet {
            assert_eq!(parsed_midi_packet.sequence_number(), 1);
            assert_eq!(parsed_midi_packet.timestamp(), 2);
            assert_eq!(parsed_midi_packet.ssrc(), 3);
            let values = parsed_midi_packet.commands().collect::<Vec<_>>();
            assert_eq!(values.len(), 1);
            assert_eq!(
                values[0].command(),
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
        let packet = SessionInitiationPacket::new_acknowledgment(1, 1, "Hello".to_string());
        let mut bytes = Vec::new();
        let result = packet.write(&mut bytes);
        assert!(result.is_ok());
        let parsed = RtpMidiPacket::parse(&bytes).unwrap();

        match parsed {
            RtpMidiPacket::Control(ControlPacket::SessionInitiation(_)) => {
                // all good
            }
            _ => panic!("Expected ControlPacket"),
        }
    }
}

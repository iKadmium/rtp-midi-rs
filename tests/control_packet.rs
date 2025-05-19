#[cfg(test)]
mod tests {
    use rtpmidi::packet::control_packets::control_packet::ControlPacket;

    #[test]
    fn test_read_clock_sync_packet_2() {
        let buffer = [
            0xFF, 0xFF, 0x43, 0x4B, //header
            0xF5, 0x19, 0xAE, 0xB9, //sender ssrc
            0x02, //count
            0x00, 0x00, 0x00, //reserved
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, // timestamp 1
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, // timestamp 2
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03, // timestamp 3
        ]; // Example buffer for a ClockSync packet

        let result = ControlPacket::from_be_bytes(&buffer);
        assert!(result.is_ok());
        if let ControlPacket::ClockSync(packet) = result.unwrap() {
            assert_eq!(packet.count, 2);
            assert_eq!(packet.sender_ssrc, 4112101049);
            assert_eq!(packet.timestamps[0], 1);
            assert_eq!(packet.timestamps[1], 2);
            assert_eq!(packet.timestamps[2], 3);
        } else {
            panic!("Expected ClockSync packet");
        }
    }

    #[test]
    fn test_read_session_initiation_packet() {
        let buffer = [
            0xFF, 0xFF, 0x49, 0x4E, //header
            0x00, 0x00, 0x00, 0x02, //version
            0xF8, 0xD1, 0x80, 0xE6, //initiator token
            0xF5, 0x19, 0xAE, 0xB9, //sender ssrc
            0x4C, 0x6F, 0x76, 0x65, 0x6C, 0x79, 0x20, 0x53, 0x65, 0x73, 0x73, 0x69, 0x6F, 0x6E,
            0x00, //name
        ];

        let result = ControlPacket::from_be_bytes(&buffer);
        assert!(result.is_ok());
        if let ControlPacket::SessionInitiation(_packet) = result.unwrap() {
            // all good!
        } else {
            panic!("Expected SessionInitiation packet");
        }
    }
}

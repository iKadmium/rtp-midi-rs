use bytes::{BufMut, Bytes, BytesMut};
use std::{
    ffi::CStr,
    io::{Error, ErrorKind},
};
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout, Unaligned, network_endian::U32};

use crate::packets::control_packets::session_initiation_packet::SessionInitiationPacketBody;

use super::clock_sync_packet::ClockSyncPacket;

const CONTROL_PACKET_MARKER: [u8; 2] = [255, 255];

#[derive(Debug, KnownLayout, Unaligned, IntoBytes, Immutable, FromBytes)]
#[repr(C)]
pub struct ControlPacketHeader {
    marker: [u8; 2],
    pub command: [u8; 2],
}

impl ControlPacketHeader {
    pub fn new(command: [u8; 2]) -> ControlPacketHeader {
        ControlPacketHeader {
            marker: CONTROL_PACKET_MARKER,
            command,
        }
    }
}

#[derive(Debug)]
pub enum ControlPacket<'a> {
    ClockSync(&'a ClockSyncPacket),
    Invitation { body: &'a SessionInitiationPacketBody, name: &'a CStr },
    Acceptance { body: &'a SessionInitiationPacketBody, name: &'a CStr },
    Rejection(&'a SessionInitiationPacketBody),
    Termination(&'a SessionInitiationPacketBody),
}

impl ControlPacket<'_> {
    pub fn from_be_bytes(buffer: &[u8]) -> std::io::Result<ControlPacket> {
        let (header, remainder) =
            ControlPacketHeader::ref_from_prefix(buffer).map_err(|_| Error::new(ErrorKind::InvalidData, "Invalid control packet header"))?;

        match &header.command {
            b"CK" => {
                let clock_sync_packet =
                    ClockSyncPacket::ref_from_bytes(remainder).map_err(|_| Error::new(ErrorKind::InvalidData, "Invalid clock sync packet"))?;
                Ok(ControlPacket::ClockSync(clock_sync_packet))
            }
            b"OK" | b"IN" => {
                let (body, payload) =
                    SessionInitiationPacketBody::ref_from_prefix(remainder).map_err(|_| Error::new(ErrorKind::InvalidData, "Invalid acceptance packet"))?;
                let name = CStr::from_bytes_with_nul(payload).map_err(|_| Error::new(ErrorKind::InvalidData, "Invalid name in acceptance packet"))?;
                if header.command == *b"OK" {
                    Ok(ControlPacket::Acceptance { body, name })
                } else {
                    Ok(ControlPacket::Invitation { body, name })
                }
            }
            b"NO" | b"BY" => {
                let body =
                    SessionInitiationPacketBody::ref_from_bytes(remainder).map_err(|_| Error::new(ErrorKind::InvalidData, "Invalid rejection packet"))?;
                if header.command == *b"NO" {
                    Ok(ControlPacket::Rejection(body))
                } else {
                    Ok(ControlPacket::Termination(body))
                }
            }
            _ => Err(Error::new(
                ErrorKind::InvalidData,
                format!("Unknown control packet, {}", String::from_utf8_lossy(&header.command)),
            ))?,
        }
    }

    pub fn is_control_packet(buffer: &[u8]) -> bool {
        buffer.starts_with(&CONTROL_PACKET_MARKER)
    }

    fn new_initiator(initiator_token: U32, sender_ssrc: U32, command: [u8; 2], name: Option<&CStr>) -> Bytes {
        let header = ControlPacketHeader::new(command);
        let packet = SessionInitiationPacketBody::new(initiator_token, sender_ssrc);
        let name_length = name.map_or(0, |n| n.count_bytes() + 1); // +1 for null terminator
        let mut buffer = BytesMut::with_capacity(std::mem::size_of::<ControlPacketHeader>() + std::mem::size_of::<SessionInitiationPacketBody>() + name_length);
        buffer.put_slice(header.as_bytes());
        buffer.put_slice(packet.as_bytes());
        if let Some(name) = name {
            buffer.put_slice(name.to_bytes_with_nul());
        }
        buffer.freeze()
    }

    pub fn new_acceptance(initiator_token: U32, sender_ssrc: U32, name: &CStr) -> Bytes {
        ControlPacket::new_initiator(initiator_token, sender_ssrc, *b"OK", Some(name))
    }

    pub fn new_invitation(initiator_token: U32, sender_ssrc: U32, name: &CStr) -> Bytes {
        ControlPacket::new_initiator(initiator_token, sender_ssrc, *b"IN", Some(name))
    }

    pub fn new_rejection(initiator_token: U32, sender_ssrc: U32) -> Bytes {
        ControlPacket::new_initiator(initiator_token, sender_ssrc, *b"NO", None)
    }

    pub fn new_termination(initiator_token: U32, sender_ssrc: U32) -> Bytes {
        ControlPacket::new_initiator(initiator_token, sender_ssrc, *b"BY", None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_invalid_control_packet() {
        let data = vec![0, 0, 0, 0];
        let result = ControlPacket::from_be_bytes(&data);
        assert!(result.is_err());
        if let Err(e) = result {
            assert_eq!(e.kind(), ErrorKind::InvalidData);
        }
    }

    #[test]
    fn test_parse_too_short_control_packet() {
        let data = vec![255, 255, 67];
        let result = ControlPacket::from_be_bytes(&data);
        assert!(result.is_err());
        if let Err(e) = result {
            assert_eq!(e.kind(), ErrorKind::InvalidData);
        }
    }

    #[test]
    fn test_is_control_packet() {
        let valid_packet = vec![255, 255, 67, 75];
        let invalid_packet = vec![0, 0, 0, 0];
        assert!(ControlPacket::is_control_packet(&valid_packet));
        assert!(!ControlPacket::is_control_packet(&invalid_packet));
    }

    #[test]
    fn test_parse_unknown_control_packet() {
        let data = vec![255, 255, 0, 0];
        let result = ControlPacket::from_be_bytes(&data);
        assert!(result.is_err());
        if let Err(e) = result {
            assert_eq!(e.kind(), ErrorKind::InvalidData);
            assert_eq!(e.to_string(), "Unknown control packet, \u{0}\u{0}");
        }
    }

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
            0x4C, 0x6F, 0x76, 0x65, 0x6C, 0x79, 0x20, 0x53, 0x65, 0x73, 0x73, 0x69, 0x6F, 0x6E, 0x00, //name
        ];

        let result = ControlPacket::from_be_bytes(&buffer);
        assert!(result.is_ok());
        if let ControlPacket::Invitation { body: _, name: _ } = result.unwrap() {
            // all good!
        } else {
            panic!("Expected Invitation packet");
        }
    }
}

pub mod control_port;
pub mod invite_response;
mod mdns;
pub mod midi_port;
pub mod rtp_midi_session;
mod rtp_port;

const MAX_UDP_PACKET_SIZE: usize = 65535;

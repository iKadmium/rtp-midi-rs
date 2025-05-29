pub mod control_port;
mod host_syncer;
pub mod invite_responder;
mod mdns;
pub mod midi_port;
pub mod rtp_midi_session;
mod rtp_port;

const MAX_UDP_PACKET_SIZE: usize = 65535;

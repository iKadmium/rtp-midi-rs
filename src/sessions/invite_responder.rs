use std::net::SocketAddr;

use crate::packets::control_packets::session_initiation_packet::SessionInitiationPacket;

pub type InviteHandler = dyn Fn(&SessionInitiationPacket, &SocketAddr) -> bool + Send + Sync + 'static;

pub enum InviteResponder {
    Accept,
    Reject,
    Custom(Box<InviteHandler>),
}

impl InviteResponder {
    pub fn handle(&self, packet: &SessionInitiationPacket, addr: &SocketAddr) -> bool {
        match self {
            InviteResponder::Accept => true,
            InviteResponder::Reject => false,
            InviteResponder::Custom(handler) => handler(packet, addr),
        }
    }

    pub fn new<F>(handler: F) -> InviteResponder
    where
        F: Fn(&SessionInitiationPacket, &SocketAddr) -> bool + Send + Sync + 'static,
    {
        InviteResponder::Custom(Box::new(handler))
    }
}

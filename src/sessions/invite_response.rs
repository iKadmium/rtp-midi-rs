use std::net::SocketAddr;

use crate::packets::control_packets::session_initiation_packet::SessionInitiationPacket;

pub type InviteHandler = dyn Fn(&SessionInitiationPacket, &SocketAddr) -> bool + Send + Sync + 'static;

pub enum InviteResponse {
    Accept,
    Reject,
    Custom(Box<InviteHandler>),
}

impl InviteResponse {
    pub fn handle(&self, packet: &SessionInitiationPacket, addr: &SocketAddr) -> bool {
        match self {
            InviteResponse::Accept => true,
            InviteResponse::Reject => false,
            InviteResponse::Custom(handler) => handler(packet, addr),
        }
    }

    pub fn new<F>(handler: F) -> InviteResponse
    where
        F: Fn(&SessionInitiationPacket, &SocketAddr) -> bool + Send + Sync + 'static,
    {
        InviteResponse::Custom(Box::new(handler))
    }
}

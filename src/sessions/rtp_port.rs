use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use tokio::{net::UdpSocket, sync::Mutex};
use tracing::{Level, event, instrument};

use crate::{packets::control_packets::session_initiation_packet::SessionInitiationPacket, participant::Participant};

pub(super) trait RtpPort {
    fn session_name(&self) -> &str;
    fn ssrc(&self) -> u32;
    fn socket(&self) -> &Arc<UdpSocket>;

    #[instrument(skip_all, fields(destination = %destination))]
    async fn send_invitation_acceptance(&self, packet: &SessionInitiationPacket, destination: SocketAddr) {
        let response_packet = SessionInitiationPacket::new_acknowledgment(packet.initiator_token(), self.ssrc(), self.session_name().to_string());

        if let Err(e) = self.socket().send_to(&response_packet.to_bytes(), destination).await {
            event!(Level::ERROR, "Failed to send invitation response: {}", e);
        } else {
            event!(Level::INFO, "Sent invitation acceptance");
        }
    }

    #[instrument(skip_all, fields(ssrc = ssrc, src = %src))]
    async fn handle_termination(&self, ssrc: u32, src: SocketAddr, participants: &Arc<Mutex<HashMap<u32, Participant>>>) {
        event!(Level::INFO, "Received termination packet");
        let mut lock = participants.lock().await;
        lock.remove(&ssrc);
    }

    #[instrument(skip_all, fields(destination = %participant.addr(), participant = participant.name()))]
    async fn send_termination_packet(&self, participant: &Participant) {
        let termination_packet = SessionInitiationPacket::new_termination(participant.initiator_token().unwrap(), self.ssrc());
        if let Err(e) = self.socket().send_to(&termination_packet.to_bytes(), participant.addr()).await {
            event!(Level::WARN, "Failed to send termination packet: {}", e);
        } else {
            event!(Level::INFO, "Sent termination packet");
        }
    }
}

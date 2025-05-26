use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use log::{error, info};
use tokio::{net::UdpSocket, sync::Mutex};

use crate::{
    packets::control_packets::session_initiation_packet::{SessionInitiationPacket, SessionInitiationPacketBody},
    participant::Participant,
};

pub(super) trait RtpPort {
    fn log_context(&self) -> &str;
    fn session_name(&self) -> &str;
    fn ssrc(&self) -> u32;

    async fn send_invitation_acceptance(&self, packet: &SessionInitiationPacketBody, src: SocketAddr, socket: &Arc<UdpSocket>) {
        let response_packet = SessionInitiationPacket::new_acknowledgment(packet.initiator_token, self.ssrc(), self.session_name().to_string());

        if let Err(e) = socket.send_to(&response_packet.to_bytes(), src).await {
            error!("{}: Failed to send invitation response to {}: {}", self.log_context(), src, e);
        } else {
            info!("{}: Sent invitation response to {}", self.log_context(), src);
        }
    }

    async fn handle_end_session(&self, ssrc: u32, participants: &Arc<Mutex<HashMap<u32, Participant>>>) {
        info!("{}: Ending session with ssrc {}", self.log_context(), ssrc);
        let mut lock = participants.lock().await;
        lock.remove(&ssrc);
    }
}

use super::MAX_UDP_PACKET_SIZE;
use super::invite_responder::InviteResponder;
use super::rtp_midi_session::RtpMidiSession;
use super::rtp_port::RtpPort;
use crate::packets::control_packets::control_packet::ControlPacket;
use crate::packets::control_packets::session_initiation_packet::SessionInitiationPacket;
use crate::packets::control_packets::session_initiation_packet::SessionInitiationPacketBody;
use crate::sessions::rtp_midi_session::PendingInvitation;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tracing::Level;
use tracing::event;
use tracing::instrument;

pub(super) struct ControlPort {
    ssrc: u32,
    session_name: String,
    socket: Arc<UdpSocket>,
}

impl RtpPort for ControlPort {
    fn session_name(&self) -> &str {
        &self.session_name
    }

    fn ssrc(&self) -> u32 {
        self.ssrc
    }

    fn socket(&self) -> &Arc<UdpSocket> {
        &self.socket
    }
}

impl ControlPort {
    pub async fn bind(port: u16, name: &str, ssrc: u32) -> std::io::Result<Self> {
        let socket = Arc::new(UdpSocket::bind((std::net::Ipv4Addr::UNSPECIFIED, port)).await?);
        Ok(ControlPort {
            session_name: name.to_string(),
            ssrc,
            socket,
        })
    }

    #[instrument(skip_all, fields(name = %ctx.name(), addr = %addr))]
    pub async fn invite_participant(&self, ctx: &RtpMidiSession, addr: SocketAddr) {
        let initiator_token = rand::random::<u32>();
        let invitation = SessionInitiationPacket::new_invitation(initiator_token, self.ssrc, self.session_name.clone());
        let result = self.socket.send_to(&invitation.to_bytes(), addr).await;
        if let Err(e) = result {
            event!(Level::ERROR, "Failed to send session invitation: {}", e);
            return;
        }
        event!(Level::INFO, "Sent session invitation");
        ctx.pending_invitations.lock().await.insert(
            0,
            PendingInvitation {
                addr,
                token: initiator_token,
                name: String::new(),
            },
        );
    }

    #[instrument(skip_all, name = "CTRL", fields(name = %self.session_name, src))]
    pub async fn start(&self, ctx: &RtpMidiSession, invite_handler: &InviteResponder, buf: &mut [u8; MAX_UDP_PACKET_SIZE]) {
        let recv = self.socket.recv_from(buf).await;

        if recv.is_err() {
            event!(Level::ERROR, "Failed to receive data on control port: {}", recv.unwrap_err());
            return;
        }

        let (amt, src) = recv.unwrap();
        tracing::Span::current().record("src", src.to_string());
        event!(Level::TRACE, "Received {} bytes", amt);

        let maybe_ctrl_packet = ControlPacket::from_be_bytes(&buf[..amt]);
        if maybe_ctrl_packet.is_err() {
            event!(Level::WARN, "Failed to parse control packet: {}", maybe_ctrl_packet.unwrap_err());
            return;
        }

        let packet = maybe_ctrl_packet.unwrap();
        event!(Level::TRACE, packet = std::format!("{:?}", packet), "Parsed packet");

        match packet {
            ControlPacket::SessionInitiation(session_initiation_packet) => match &session_initiation_packet {
                SessionInitiationPacket::Invitation(_) => {
                    self.handle_invitation(&session_initiation_packet, invite_handler, ctx, src).await;
                }
                SessionInitiationPacket::Acknowledgment(ack_body) => {
                    self.handle_acknowledgment(ack_body, ctx, src).await;
                }
                SessionInitiationPacket::Rejection(rejection) => {
                    self.handle_rejection(rejection, ctx, src).await;
                }
                SessionInitiationPacket::Termination(body) => {
                    self.handle_termination(body.sender_ssrc, src, &ctx.participants).await;
                }
            },
            _ => {
                event!(Level::WARN, packet = std::format!("{:?}", packet), "Control: Unhandled control packet");
            }
        }
    }

    #[instrument(skip_all)]
    async fn handle_invitation(&self, invitation: &SessionInitiationPacket, invite_handler: &InviteResponder, ctx: &RtpMidiSession, src: SocketAddr) {
        event!(Level::INFO, token = invitation.initiator_token(), "Received session invitation");
        let accept = invite_handler.handle(invitation, &src);
        if accept {
            event!(Level::INFO, "Accepted session invitation");
            ctx.pending_invitations.lock().await.insert(
                invitation.ssrc(),
                PendingInvitation {
                    addr: src,
                    token: invitation.initiator_token(),
                    name: invitation.name().unwrap_or("No name").to_string(),
                },
            );
            self.send_invitation_acceptance(invitation, src).await;
        } else {
            event!(Level::INFO, "Rejected session initiation");
            let rejection_packet = SessionInitiationPacket::new_rejection(invitation.initiator_token(), self.ssrc, self.session_name.clone());
            let result = self.socket.send_to(&rejection_packet.to_bytes(), src).await;
            if let Err(e) = result {
                event!(Level::ERROR, "Failed to send session rejection: {}", e);
            } else {
                event!(Level::DEBUG, "Sent session rejection");
            }
        }
    }

    #[instrument(skip_all, fields(token = rejection.initiator_token))]
    async fn handle_rejection(&self, rejection: &SessionInitiationPacketBody, ctx: &RtpMidiSession, src: SocketAddr) {
        event!(Level::INFO, "Received session rejection");
        let _ = self.remove_invitation(rejection, ctx, src).await;
    }

    #[instrument(skip_all)]
    async fn remove_invitation(&self, invitation_response: &SessionInitiationPacketBody, ctx: &RtpMidiSession, src: SocketAddr) -> Option<PendingInvitation> {
        event!(Level::DEBUG, "Removing invitation for SSRC {} at {}", invitation_response.sender_ssrc, src);
        let mut locked_pending_invitations = ctx.pending_invitations.lock().await;
        if locked_pending_invitations.contains_key(&invitation_response.sender_ssrc) {
            locked_pending_invitations.remove(&invitation_response.sender_ssrc)
        } else if !locked_pending_invitations.contains_key(&invitation_response.sender_ssrc)
            && locked_pending_invitations.contains_key(&0)
            && locked_pending_invitations[&0].token == invitation_response.initiator_token
            && locked_pending_invitations[&0].addr == src
        {
            locked_pending_invitations.remove(&0)
        } else {
            None
        }
    }

    #[instrument(skip_all, fields(ssrc = ack_body.sender_ssrc, src = %src))]
    async fn handle_acknowledgment(&self, ack_body: &SessionInitiationPacketBody, ctx: &RtpMidiSession, src: SocketAddr) {
        event!(Level::INFO, "Received session acknowledgment");
        let inv = self.remove_invitation(ack_body, ctx, src).await;
        if inv.is_none() {
            event!(Level::WARN, "Received Acknowledgment but no matching invitation found");
            return;
        }

        let inv = inv.unwrap();
        if inv.token != ack_body.initiator_token {
            event!(
                Level::WARN,
                "Received Acknowledgment from {} with mismatched token. Expected {}, got {}.",
                inv.addr,
                inv.token,
                ack_body.initiator_token
            );
        }

        event!(
            Level::DEBUG,
            "Matched Acknowledgment from {} invitation. Sending MIDI port invitation.",
            inv.addr
        );

        let midi_addr = SocketAddr::new(inv.addr.ip(), inv.addr.port() + 1);

        let mut lock = ctx.pending_invitations.lock().await;
        lock.insert(
            ack_body.sender_ssrc,
            PendingInvitation {
                addr: midi_addr,
                token: inv.token,
                name: ack_body.name.clone().unwrap_or_else(|| "No name".to_string()),
            },
        );

        let response_packet = SessionInitiationPacket::new_invitation(inv.token, self.ssrc, self.session_name.clone());
        ctx.midi_port.send_invitation(&response_packet, midi_addr).await;
    }
}

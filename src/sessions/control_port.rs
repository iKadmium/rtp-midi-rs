use super::MAX_UDP_PACKET_SIZE;
use super::invite_responder::InviteResponder;
use super::rtp_midi_session::RtpMidiSession;
use super::rtp_port::RtpPort;
use crate::packets::control_packets::control_packet::ControlPacket;
use crate::packets::control_packets::session_initiation_packet::SessionInitiationPacket;
use crate::packets::control_packets::session_initiation_packet::SessionInitiationPacketBody;
use crate::participant::Participant;
use crate::sessions::rtp_midi_session::PendingInvitation;
use log::{debug, error, info, trace, warn};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio_util::sync::CancellationToken;

pub(super) struct ControlPort {
    ssrc: u32,
    session_name: String,
    log_context: String,
    socket: Arc<UdpSocket>,
    cancel_token: CancellationToken,
}

impl RtpPort for ControlPort {
    fn log_context(&self) -> &str {
        self.log_context.as_str()
    }

    fn session_name(&self) -> &str {
        &self.session_name
    }

    fn ssrc(&self) -> u32 {
        self.ssrc
    }
}

impl ControlPort {
    pub async fn bind(port: u16, name: &str, ssrc: u32) -> std::io::Result<Self> {
        let socket = Arc::new(UdpSocket::bind((std::net::Ipv4Addr::UNSPECIFIED, port)).await?);
        Ok(ControlPort {
            session_name: name.to_string(),
            ssrc,
            log_context: format!("{}-CTRL", name),
            socket,
            cancel_token: CancellationToken::new(),
        })
    }

    pub fn stop(&self) {
        self.cancel_token.cancel();
    }

    pub async fn invite_participant(&self, ctx: &RtpMidiSession, addr: SocketAddr) -> std::io::Result<()> {
        let initiator_token = rand::random::<u32>();
        let invitation = SessionInitiationPacket::new_invitation(initiator_token, self.ssrc, self.session_name.clone());
        self.socket.send_to(&invitation.to_bytes(), addr).await?;
        info!("{}-Control: Sent session invitation to {}", self.session_name, addr);
        ctx.pending_invitations.lock().await.insert(
            0,
            PendingInvitation {
                addr,
                token: initiator_token,
                name: String::new(),
            },
        );
        Ok(())
    }

    pub async fn start(&self, ctx: &RtpMidiSession, invite_handler: &InviteResponder) {
        let mut buf = [0; MAX_UDP_PACKET_SIZE];
        loop {
            tokio::select! {
                _ = self.cancel_token.cancelled() => {
                    debug!("listen_for_control: cancellation requested");
                    break;
                },
                recv = self.socket.recv_from(&mut buf) => {
                    match recv {
                        Ok((amt, src)) => {
                            trace!("{}: Received {} bytes from {}", self.log_context(), amt, src);
                            match ControlPacket::from_be_bytes(&buf[..amt]) {
                                Ok(packet) => {
                                    trace!("{}: Parsed packet: {:?}", self.log_context(), packet);
                                    match packet {
                                        ControlPacket::SessionInitiation(session_initiation_packet) => match &session_initiation_packet {
                                            SessionInitiationPacket::Invitation(invitation) => {
                                                let accept = invite_handler.handle(&session_initiation_packet, &src);
                                                if accept {
                                                    info!("{}: Accepted session initiation from {}", self.log_context(), src);
                                                    ctx.pending_invitations.lock().await.insert(
                                                        invitation.sender_ssrc,
                                                        PendingInvitation {
                                                            addr: src,
                                                            token: invitation.initiator_token,
                                                            name: invitation.name.clone().unwrap_or_default(),
                                                        },
                                                    );
                                                    self.send_invitation_acceptance(invitation, src, &self.socket).await;
                                                } else {
                                                    info!("{}: Rejected session initiation from {}", self.log_context(), src);
                                                    let rejection_packet = SessionInitiationPacket::new_rejection(invitation.initiator_token, self.ssrc, self.session_name.clone());
                                                    let _ = self.socket.send_to(&rejection_packet.to_bytes(), src).await;
                                                }
                                            }
                                            SessionInitiationPacket::Acknowledgment(ack_body) => {
                                                info!("{}: Received session acknowledgment from {} for token {}",self.log_context(), src, ack_body.initiator_token);
                                                ControlPort::handle_acknowledgment(self, ack_body, ctx, src).await;
                                            }
                                            SessionInitiationPacket::Rejection(_) => {
                                                info!("{}: Received session rejection from {}", self.log_context(),src);
                                            }
                                            SessionInitiationPacket::Termination(body) => {
                                                info!("{}: Received session termination from {} (ssrc={})", self.log_context(),src, body.sender_ssrc);
                                                self.handle_end_session(body.sender_ssrc, &ctx.participants).await;
                                            }
                                        },
                                        _ => {
                                            warn!("Control: Unhandled control packet: {:?}", packet);
                                        }
                                    }
                                }
                                Err(e) => {
                                    warn!("{}: {} from {}", self.log_context(), e, src);
                                }
                            }
                        }
                        Err(e) => {
                            error!("{}: Error receiving data: {}", self.log_context(), e);
                            break;
                        }
                    }
                }
            }
        }
    }

    async fn handle_acknowledgment(&self, ack_body: &SessionInitiationPacketBody, ctx: &RtpMidiSession, src: SocketAddr) {
        let src_ssrc = ack_body.sender_ssrc;
        let mut locked_pending_invitations = ctx.pending_invitations.lock().await;
        // If we don't have an entry for this SSRC, but we have a placeholder (0), update the key
        if !locked_pending_invitations.contains_key(&src_ssrc)
            && locked_pending_invitations.contains_key(&0)
            && locked_pending_invitations[&0].token == ack_body.initiator_token
            && locked_pending_invitations[&0].addr == src
        {
            // Move the entry to the new SSRC
            let inv = locked_pending_invitations.remove(&0).unwrap();
            let new_inv = PendingInvitation {
                addr: inv.addr,
                token: inv.token,
                name: ack_body.name.clone().unwrap_or_default(),
            };
            locked_pending_invitations.insert(src_ssrc, new_inv);
        }
        if let Some(inv) = locked_pending_invitations.get(&src_ssrc).cloned() {
            if inv.token == ack_body.initiator_token {
                locked_pending_invitations.remove(&src_ssrc);
                drop(locked_pending_invitations);
                debug!(
                    "{}: Matched Acknowledgment from {} invitation. Sending MIDI port invitation.",
                    self.log_context(),
                    inv.addr
                );
                let response_packet = SessionInitiationPacket::new_invitation(inv.token, self.ssrc, inv.name.clone());
                let midi_addr = SocketAddr::new(inv.addr.ip(), inv.addr.port() + 1);
                if let Err(e) = ctx.midi_port.send_invitation(&response_packet, midi_addr).await {
                    warn!("{}: Failed to send MIDI port invitation to {}: {}", self.log_context(), midi_addr, e);
                } else {
                    info!("{}: Sent MIDI port invitation to {} with token {}", self.log_context(), midi_addr, inv.token);
                    ctx.pending_invitations.lock().await.insert(
                        src_ssrc,
                        PendingInvitation {
                            addr: midi_addr,
                            token: inv.token,
                            name: inv.name,
                        },
                    );
                }
            } else {
                warn!(
                    "{}: Received Acknowledgment from {} with mismatched token. Expected {}, got {}.",
                    self.log_context(),
                    inv.addr,
                    inv.token,
                    ack_body.initiator_token
                );
            }
        } else {
            warn!(
                "{}: Received Acknowledgment from {} but no pending invitation found for this SSRC.",
                self.log_context(),
                src_ssrc
            );
        }
    }

    pub(super) async fn send_termination_packet(&self, participant: &Participant) -> std::io::Result<()> {
        let termination_packet = SessionInitiationPacket::new_termination(participant.initiator_token().unwrap(), self.ssrc);
        if let Err(e) = self.socket.send_to(&termination_packet.to_bytes(), participant.addr()).await {
            warn!("{}: Failed to send termination packet to {}: {}", self.log_context(), participant.addr(), e);
            return Err(e);
        } else {
            info!("{}: Sent termination packet to {}", self.log_context(), participant.addr());
        }
        Ok(())
    }
}

impl Drop for ControlPort {
    fn drop(&mut self) {
        self.stop();
    }
}

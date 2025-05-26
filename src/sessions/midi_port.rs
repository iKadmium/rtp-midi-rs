use super::MAX_UDP_PACKET_SIZE;
use super::rtp_midi_session::{ListenerSet, RtpMidiSession, current_timestamp};
use super::rtp_port::RtpPort;
use crate::packets::control_packets::clock_sync_packet::ClockSyncPacket;
use crate::packets::control_packets::control_packet::ControlPacket;
use crate::packets::control_packets::session_initiation_packet::{SessionInitiationPacket, SessionInitiationPacketBody};
use crate::packets::midi_packets::midi_command::MidiCommand;
use crate::packets::midi_packets::midi_packet::MidiPacket;
use crate::packets::midi_packets::midi_timed_command::TimedCommand;
use crate::packets::packet::RtpMidiPacket;
use crate::participant::Participant;
use crate::sessions::rtp_midi_session::RtpMidiEventType;
use log::{debug, error, info, trace, warn};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::UdpSocket;
use tokio::sync::Mutex;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

impl RtpPort for MidiPort {
    fn log_context(&self) -> &str {
        &self.log_context
    }

    fn session_name(&self) -> &str {
        &self.name
    }

    fn ssrc(&self) -> u32 {
        self.ssrc
    }
}

pub(super) struct MidiPort {
    name: String,
    log_context: String,
    ssrc: u32,
    cancel_token: CancellationToken,
    socket: Arc<UdpSocket>,
}

impl MidiPort {
    pub async fn bind(port: u16, name: &str, ssrc: u32) -> std::io::Result<Self> {
        let socket = Arc::new(UdpSocket::bind((std::net::Ipv4Addr::UNSPECIFIED, port)).await?);
        Ok(MidiPort {
            ssrc,
            name: name.to_string(),
            log_context: format!("{}-MIDI", name),
            socket,
            cancel_token: CancellationToken::new(),
        })
    }

    pub fn stop(&self) {
        self.cancel_token.cancel();
    }

    pub async fn start_listener(&self, ctx: &RtpMidiSession, listeners: Arc<Mutex<ListenerSet>>) {
        let port_identifier = self.log_context();
        let mut buf = [0; MAX_UDP_PACKET_SIZE];
        loop {
            tokio::select! {
                _ = self.cancel_token.cancelled() => {
                    debug!("listen_for_midi: cancellation requested");
                    break;
                },
                recv = self.socket.recv_from(&mut buf) => {
                    match recv {
                        Ok((amt, src)) => {
                            trace!("{}: Received {} bytes from {}", port_identifier, amt, src);
                            match crate::packets::packet::RtpMidiPacket::parse(&buf[..amt]) {
                                Ok(packet) => {
                                    trace!("{}: Parsed RTP MIDI packet: {:?}", port_identifier, packet);
                                    match packet {
                                        RtpMidiPacket::Control(control_packet) => match control_packet {
                                            ControlPacket::SessionInitiation(session_initiation_packet) => match session_initiation_packet {
                                                SessionInitiationPacket::Invitation(invitation) => {
                                                    info!("{}: Received session invitation from {}", port_identifier, src);
                                                    let ctrl_addr = SocketAddr::new(src.ip(), src.port() - 1);
                                                    ctx.participants.lock().await.insert(
                                                        invitation.sender_ssrc,
                                                        Participant::new(ctrl_addr, false, Some(invitation.initiator_token), invitation.name.clone().unwrap_or_default(), invitation.sender_ssrc),
                                                    );
                                                    self.send_invitation_acceptance(&invitation, src, &self.socket).await;
                                                }
                                                SessionInitiationPacket::Acknowledgment(ack_body) => {
                                                    info!("{}: Received session acknowledgment from {} for token {}", port_identifier, src, ack_body.initiator_token);
                                                    self.handle_acknowledgment(&ack_body, ctx).await;
                                                }
                                                _ => {
                                                    warn!("{}: Unhandled session initiation packet {:?}", port_identifier, session_initiation_packet);
                                                }
                                            },
                                            ControlPacket::ClockSync(clock_sync_packet) => {
                                                debug!("{}: Received clock sync from {}", port_identifier, src);
                                                self.handle_clock_sync(
                                                    clock_sync_packet,
                                                    src,
                                                    ctx
                                                ).await;
                                            }
                                        },
                                        RtpMidiPacket::Midi(midi_packet) => {
                                            debug!("{}: Parsed MIDI packet: {:#?}", port_identifier ,midi_packet);
                                            let mut seq = ctx.sequence_number.lock().await;
                                            *seq = midi_packet.sequence_number().wrapping_add(1);
                                            if let Some(callback) = listeners.lock().await.get(&RtpMidiEventType::MidiPacket) {
                                                callback(midi_packet);
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    error!("{}: Failed to parse RTP MIDI packet: {}", port_identifier, e);
                                }
                            }
                        }
                        Err(e) => {
                            error!("{}: Error receiving data: {}", port_identifier, e);
                            break;
                        }
                    }
                }
            }
        }
    }

    async fn handle_acknowledgment(&self, ack_body: &SessionInitiationPacketBody, ctx: &RtpMidiSession) {
        let src_ssrc = ack_body.sender_ssrc;
        let mut locked_pending_invitations = ctx.pending_invitations.lock().await;
        if let Some(inv) = locked_pending_invitations.get(&src_ssrc).cloned() {
            if inv.token == ack_body.initiator_token {
                locked_pending_invitations.remove(&src_ssrc);
                drop(locked_pending_invitations);
                debug!(
                    "{}: Matched Acknowledgment from {} for MIDI port invitation. Sending Clock Sync.",
                    self.log_context(),
                    inv.addr
                );
                let ctrl_addr = SocketAddr::new(inv.addr.ip(), inv.addr.port() - 1);
                let response_packet = ClockSyncPacket::new(0, [current_timestamp(ctx.start_time), 0, 0], self.ssrc);
                ctx.participants
                    .lock()
                    .await
                    .insert(src_ssrc, Participant::new(ctrl_addr, true, Some(inv.token), inv.name, src_ssrc));
                if let Err(e) = self.socket.send_to(&response_packet.to_bytes(), inv.addr).await {
                    warn!("{}: Failed to send clock sync to {}: {}", self.log_context(), inv.addr, e);
                } else {
                    info!("{}: Sent clock sync to {}", self.log_context(), inv.addr);
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

    async fn handle_clock_sync(&self, packet: ClockSyncPacket, src: SocketAddr, ctx: &RtpMidiSession) {
        match packet.count {
            0 => {
                let timestamp2 = current_timestamp(ctx.start_time);
                let response_packet = ClockSyncPacket::new(1, [packet.timestamps[0], timestamp2, 0], self.ssrc);
                if let Err(e) = self.socket.send_to(&response_packet.to_bytes(), src).await {
                    error!("{}: Failed to send clock sync response to {}: {}", self.log_context(), src, e);
                } else {
                    debug!("{}: Sent clock sync response to {}", self.log_context(), src);
                }
            }
            1 => {
                let mut lock = ctx.participants.lock().await;
                if let Some(participant) = lock.get_mut(&packet.sender_ssrc) {
                    participant.received_clock_sync();
                    debug!("{}: Updated clock sync for existing participant {}", self.log_context(), src);
                    let timestamp3 = current_timestamp(ctx.start_time);
                    let response_packet = ClockSyncPacket::new(2, [packet.timestamps[0], packet.timestamps[1], timestamp3], self.ssrc);
                    if let Err(e) = self.socket.send_to(&response_packet.to_bytes(), src).await {
                        error!(
                            "{}: Failed to send clock sync response to {}: {}",
                            self.socket.local_addr().unwrap().port(),
                            src,
                            e
                        );
                    } else {
                        debug!("{}: Sent clock sync response to {}", self.log_context(), src);
                    }
                } else {
                    warn!(
                        "{}: Clock sync count {} received from {} but no matching participant found",
                        self.log_context(),
                        packet.count,
                        src
                    );
                }
            }
            2 => {
                let mut lock = ctx.participants.lock().await;
                if let Some(_participant) = lock.get_mut(&packet.sender_ssrc) {
                    let latency_estimate = (packet.timestamps[2] - packet.timestamps[0]) as f32 / 10.0;
                    info!(
                        "{}: Clock sync finalized with {} (latency estimate: {}ms)",
                        self.log_context(),
                        src,
                        latency_estimate
                    );
                } else {
                    warn!(
                        "{}: Clock sync count {} received from {} but no matching participant found",
                        self.log_context(),
                        packet.count,
                        src
                    );
                }
            }
            _ => {
                error!("{}: Unexpected clock sync count {} from {}", self.log_context(), packet.count, src);
            }
        }
    }

    pub async fn start_host_clock_sync(&self, ctx: &RtpMidiSession) {
        loop {
            tokio::select! {
                _ = self.cancel_token.cancelled() => {
                    debug!("start_host_clock_sync: cancellation requested");
                    break;
                }
                // Sleep for 10 seconds between syncs
                _ = sleep(Duration::from_secs(10)) => {
                    let mut lock = ctx.participants.lock().await;
                    let before_count = lock.len();
                    if before_count == 0 {
                        debug!("No participants to sync with");
                        continue;
                    }

                    let stale_participants: Vec<_> = lock
                        .iter()
                        .filter(|(_, p)| p.is_invited_by_us() && Instant::now().duration_since(p.last_clock_sync()) >= Duration::from_secs(30))
                        .map(|(ssrc, p)| (*ssrc, p.clone()))
                        .collect();

                    lock.retain(|ssrc, _| !stale_participants.iter().any(|(stale_ssrc, _)| stale_ssrc == ssrc));

                    for (_ssrc, participant) in stale_participants {
                        let _ = ctx.remove_participant(&participant).await;
                    }

                    let after_count = lock.len();
                    let removed_count = before_count - after_count;
                    if removed_count > 0 {
                        info!("Removed {} stale participant(s)", removed_count);
                    }

                    let now = current_timestamp(ctx.start_time);
                    let clock_sync = ClockSyncPacket::new(0, [now, 0, 0], self.ssrc);
                    let clock_sync_bytes = clock_sync.to_bytes();
                    for p in lock.values_mut() {
                        match self.socket.send_to(&clock_sync_bytes, p.midi_port_addr()).await {
                            Ok(_) => {
                                debug!("Sent clock sync to {}", p.midi_port_addr());
                            }
                            Err(e) => {
                                warn!("Failed to send clock sync to {}: {}", p.midi_port_addr(), e);
                            }
                        }
                    }
                }
            }
        }
    }

    pub async fn send_midi_batch(&self, ctx: &RtpMidiSession, commands: &[TimedCommand]) -> std::io::Result<()> {
        let lock = ctx.participants.lock().await;
        let participants: Vec<Participant> = lock.values().cloned().collect();
        let mut seq = ctx.sequence_number.lock().await;
        let packet = MidiPacket::new(*seq, current_timestamp(ctx.start_time) as u32, self.ssrc, commands);
        *seq = seq.wrapping_add(1);
        let packet_bytes = packet.to_bytes(false);
        debug!("{}: Sending MIDI packet batch to {:?}", self.log_context(), participants);
        for participant in participants {
            self.socket.send_to(&packet_bytes, participant.midi_port_addr()).await?;
        }
        Ok(())
    }

    pub async fn send_midi(&self, ctx: &RtpMidiSession, command: &MidiCommand) -> std::io::Result<()> {
        let batch: [TimedCommand; 1] = [TimedCommand::new(None, command.clone())];
        self.send_midi_batch(ctx, &batch).await
    }

    pub(super) async fn send_invitation(&self, invitation: &SessionInitiationPacket, addr: SocketAddr) -> std::io::Result<()> {
        let packet_bytes = invitation.to_bytes();
        debug!("{}: Sending session invitation to {}", self.log_context(), addr);
        self.socket.send_to(&packet_bytes, addr).await?;
        Ok(())
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

impl Drop for MidiPort {
    fn drop(&mut self) {
        self.stop();
    }
}

use log::{debug, error, info, trace, warn};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::UdpSocket;
use tokio::sync::Mutex;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

#[cfg(feature = "mdns")]
use mdns_sd::{ServiceDaemon, ServiceInfo};

use crate::packets::control_packets::clock_sync_packet::ClockSyncPacket;
use crate::packets::control_packets::control_packet::ControlPacket;
use crate::packets::control_packets::session_initiation_packet::{SessionInitiationPacket, SessionInitiationPacketBody};
use crate::packets::midi_packets::midi_command::MidiCommand;
use crate::packets::midi_packets::midi_packet::MidiPacket;
use crate::packets::midi_packets::midi_timed_command::TimedCommand;
use crate::packets::packet::RtpMidiPacket;
use crate::participant::Participant;

type ListenerSet = HashMap<RtpMidiEventType, Box<dyn Fn(MidiPacket) + Send>>;
type InviteHandler = dyn Fn(&SessionInitiationPacket, &SocketAddr) -> bool + Send + Sync;

pub struct RtpMidiSession {
    name: String,
    ssrc: u32,
    listeners: Arc<Mutex<ListenerSet>>,
    session_ctx: Mutex<Option<SessionContext>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RtpMidiEventType {
    MidiPacket,
}

#[derive(Clone)]
struct SessionContext {
    pub name: String,
    pub ssrc: u32,
    pub start_time: Instant,
    pub participants: Arc<Mutex<HashMap<u32, Participant>>>, // key by ssrc
    pub sequence_number: Arc<Mutex<u16>>,
    // Store both token and name for pending invitations
    pub pending_invitations: Arc<Mutex<HashMap<u32, PendingInvitation>>>, // key by ssrc
    pub midi_socket: Arc<UdpSocket>,
    pub control_socket: Arc<UdpSocket>,
    pub cancel_token: CancellationToken,
}

#[derive(Debug, Clone)]
pub struct PendingInvitation {
    pub addr: SocketAddr,
    pub token: u32,
    pub name: String,
}

impl RtpMidiSession {
    pub fn new(name: String, ssrc: u32) -> Self {
        Self {
            name,
            ssrc,
            listeners: Arc::new(Mutex::new(HashMap::new())),
            session_ctx: Mutex::new(None),
        }
    }

    fn session_context(&self, control_socket: Arc<UdpSocket>, midi_socket: Arc<UdpSocket>, cancel_token: CancellationToken) -> SessionContext {
        SessionContext {
            name: self.name.clone(),
            ssrc: self.ssrc,
            start_time: Instant::now(),
            participants: Arc::new(Mutex::new(HashMap::new())),
            sequence_number: Arc::new(Mutex::new(0)),
            pending_invitations: Arc::new(Mutex::new(HashMap::new())),
            midi_socket,
            control_socket,
            cancel_token,
        }
    }

    pub fn accept_all_invitations(_packet: &SessionInitiationPacket, _socket: &SocketAddr) -> bool {
        true
    }

    pub fn reject_all_invitations(_packet: &SessionInitiationPacket, _socket: &SocketAddr) -> bool {
        false
    }

    pub async fn add_listener<F>(&self, event_type: RtpMidiEventType, callback: F)
    where
        F: Fn(MidiPacket) + Send + 'static,
    {
        let mut listeners = self.listeners.lock().await;
        listeners.insert(event_type, Box::new(callback));
    }

    pub async fn start<F>(&self, port: u16, invite_handler: F) -> std::io::Result<()>
    where
        F: Fn(&SessionInitiationPacket, &SocketAddr) -> bool + Send + Sync + 'static,
    {
        // Advertise the service on mDNS
        Self::advertise_mdns(&self.name.clone(), port).expect("Failed to advertise service");

        let listeners_midi = Arc::clone(&self.listeners);
        let invite_handler = Arc::new(invite_handler);

        let control_socket = Arc::new(UdpSocket::bind((std::net::Ipv4Addr::UNSPECIFIED, port)).await?);
        let midi_socket = Arc::new(UdpSocket::bind((std::net::Ipv4Addr::UNSPECIFIED, port + 1)).await?);
        let cancel_token = CancellationToken::new();

        // If session_ctx is already set, return an error
        {
            let ctx = self.session_context(control_socket, midi_socket, cancel_token.clone());

            let mut session_ctx_guard = self.session_ctx.lock().await;
            if session_ctx_guard.is_some() {
                return Err(std::io::Error::new(std::io::ErrorKind::AlreadyExists, "Session context already initialized"));
            }
            *session_ctx_guard = Some(ctx.clone());

            info!("RTP MIDI session {} starting on Control Port {}, MIDI Port {}", ctx.name, port, port + 1);

            tokio::spawn(Self::listen_for_control(ctx.clone(), invite_handler.clone()));
            tokio::spawn(Self::listen_for_midi(ctx.clone(), listeners_midi.clone()));
            tokio::spawn(Self::start_host_clock_sync(ctx.clone()));
        }

        Ok(())
    }

    pub async fn stop(&self) {
        let mut session_ctx_guard = self.session_ctx.lock().await;
        if let Some(ctx) = session_ctx_guard.take() {
            ctx.cancel_token.cancel();
        }
    }

    #[cfg(feature = "mdns")]
    pub fn advertise_mdns(instance_name: &str, port: u16) -> Result<(), mdns_sd::Error> {
        let mdns = ServiceDaemon::new()?;
        let service_type = "_apple-midi._udp.local.";
        let ip = local_ip_address::local_ip().expect("Failed to get local IP address").to_string();

        let raw_hostname = hostname::get().expect("Failed to get hostname").to_string_lossy().to_string();
        let hostname = format!("{}.local.", raw_hostname);
        let service = ServiceInfo::new(service_type, instance_name, &hostname, ip, port, None)?;
        mdns.register(service)?;

        Ok(())
    }

    #[cfg(not(feature = "mdns"))]
    pub fn advertise_mdns(_: &str, _: u16) -> Result<(), std::io::Error> {
        info!("mDNS advertising is disabled. To enable it, compile with the 'mdns' feature.");
        Ok(())
    }

    async fn listen_for_control(ctx: SessionContext, invite_handler: Arc<InviteHandler>) {
        let port_identifier = format!("{}-Control", ctx.name);
        let mut buf = [0; 65535];
        loop {
            tokio::select! {
                _ = ctx.cancel_token.cancelled() => {
                    debug!("listen_for_control: cancellation requested");
                    break;
                },
                recv = ctx.control_socket.recv_from(&mut buf) => {
                    match recv {
                        Ok((amt, src)) => {
                            trace!("{}: Received {} bytes from {}", port_identifier, amt, src);
                            match ControlPacket::from_be_bytes(&buf[..amt]) {
                                Ok(packet) => {
                                    trace!("{}: Parsed packet: {:?}", port_identifier, packet);
                                    match packet {
                                        ControlPacket::SessionInitiation(session_initiation_packet) => match &session_initiation_packet {
                                            SessionInitiationPacket::Invitation(invitation) => {
                                                let accept = (invite_handler)(&session_initiation_packet, &src);
                                                if accept {
                                                    info!("{}: Accepted session initiation from {}", port_identifier, src);
                                                    // Store by remote SSRC
                                                    ctx.pending_invitations.lock().await.insert(
                                                        invitation.sender_ssrc,
                                                        PendingInvitation {
                                                            addr: src,
                                                            token: invitation.initiator_token,
                                                            name: invitation.name.clone().unwrap_or_default(),
                                                        },
                                                    );
                                                    Self::send_invitation_response(invitation, &ctx, src, &ctx.control_socket, &port_identifier).await;
                                                } else {
                                                    info!("{}: Rejected session initiation from {}", port_identifier, src);
                                                    let rejection_packet =
                                                        SessionInitiationPacket::new_rejection(invitation.initiator_token, ctx.ssrc, ctx.name.clone());
                                                    let _ = ctx.control_socket.send_to(&rejection_packet.to_bytes(), src).await;
                                                }
                                            }
                                            SessionInitiationPacket::Acknowledgment(ack_body) => {
                                                info!("{}: Received session acknowledgment from {} for token {}",port_identifier, src, ack_body.initiator_token);
                                                Self::handle_acknowledgment(true, ack_body, &ctx, &port_identifier).await;
                                            }
                                            SessionInitiationPacket::Rejection(_) => {
                                                info!("{}: Received session rejection from {}", port_identifier,src);
                                            }
                                            SessionInitiationPacket::Termination(body) => {
                                                info!("{}: Received session termination from {} (ssrc={})", port_identifier,src, body.sender_ssrc);
                                                Self::handle_end_session(body.sender_ssrc, &ctx.participants);
                                            }
                                        },
                                        _ => {
                                            warn!("Control: Unhandled control packet: {:?}", packet);
                                        }
                                    }
                                }
                                Err(e) => {
                                    warn!("{}: {} from {}", port_identifier, e, src);
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

    async fn listen_for_midi(ctx: SessionContext, listeners: Arc<Mutex<ListenerSet>>) {
        let port_identifier = std::format!("{}-MIDI", ctx.name);
        let mut buf = [0; 65535];
        loop {
            tokio::select! {
                _ = ctx.cancel_token.cancelled() => {
                    debug!("listen_for_midi: cancellation requested");
                    break;
                },
                recv = ctx.midi_socket.recv_from(&mut buf) => {
                    match recv {
                        Ok((amt, src)) => {
                            trace!("{}: Received {} bytes from {}", port_identifier, amt, src);
                            match RtpMidiPacket::parse(&buf[..amt]) {
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
                                                        Participant::new(ctrl_addr, false, Some(invitation.initiator_token), invitation.name.clone().unwrap_or_default()),
                                                    );

                                                    Self::send_invitation_response(&invitation, &ctx, src, &ctx.midi_socket, &port_identifier).await;

                                                }
                                                SessionInitiationPacket::Acknowledgment(ack_body) => {
                                                    info!("{}: Received session acknowledgment from {} for token {}", port_identifier, src, ack_body.initiator_token);
                                                    Self::handle_acknowledgment(false, &ack_body, &ctx, &port_identifier).await;
                                                }
                                                _ => {
                                                    warn!("{}: Unhandled session initiation packet {:?}", port_identifier, session_initiation_packet);
                                                }
                                            },
                                            ControlPacket::ClockSync(clock_sync_packet) => {
                                                debug!("{}: Received clock sync from {}", port_identifier, src);
                                                Self::handle_clock_sync(
                                                    &ctx.midi_socket,
                                                    clock_sync_packet,
                                                    src,
                                                    ctx.ssrc,
                                                    ctx.participants.clone(),
                                                    ctx.start_time,
                                                    &port_identifier
                                                )
                                                .await;
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

    async fn send_invitation_response(packet: &SessionInitiationPacketBody, ctx: &SessionContext, src: SocketAddr, socket: &UdpSocket, port_identifier: &str) {
        let response_packet = SessionInitiationPacket::new_acknowledgment(packet.initiator_token, ctx.ssrc, ctx.name.clone());

        if let Err(e) = socket.send_to(&response_packet.to_bytes(), src).await {
            error!("{}: Failed to send invitation response to {}: {}", port_identifier, src, e);
        } else {
            info!("{}: Sent invitation response to {}", port_identifier, src);
        }
    }

    // Update handle_end_session to take ssrc as key
    fn handle_end_session(ssrc: u32, participants: &Arc<Mutex<HashMap<u32, Participant>>>) {
        info!("Control: Ending session with ssrc {}", ssrc);
        let participants = Arc::clone(participants);
        tokio::spawn(async move {
            let mut lock = participants.lock().await;
            lock.remove(&ssrc);
        });
    }

    async fn handle_clock_sync(
        socket: &UdpSocket,
        packet: ClockSyncPacket,
        src: SocketAddr,
        local_ssrc: u32,
        participants: Arc<Mutex<HashMap<u32, Participant>>>,
        start_time: Instant, // Added start_time parameter
        port_identifier: &str,
    ) {
        match packet.count {
            0 => {
                // Respond with count = 1
                let timestamp2 = Self::current_timestamp(start_time); // Use start_time
                let response_packet = ClockSyncPacket::new(1, [packet.timestamps[0], timestamp2, 0], local_ssrc);

                if let Err(e) = socket.send_to(&response_packet.to_bytes(), src).await {
                    error!("{}: Failed to send clock sync response to {}: {}", port_identifier, src, e);
                } else {
                    debug!("{}: Sent clock sync response to {}", port_identifier, src);
                }
            }
            1 => {
                let mut lock = participants.lock().await;
                if let Some(participant) = lock.get_mut(&packet.sender_ssrc) {
                    participant.received_clock_sync();
                    debug!("{}: Updated clock sync for existing participant {}", port_identifier, src);
                    let timestamp3 = Self::current_timestamp(start_time);
                    let response_packet = ClockSyncPacket::new(2, [packet.timestamps[0], packet.timestamps[1], timestamp3], local_ssrc);
                    if let Err(e) = socket.send_to(&response_packet.to_bytes(), src).await {
                        error!("{}: Failed to send clock sync response to {}: {}", socket.local_addr().unwrap().port(), src, e);
                    } else {
                        debug!("{}: Sent clock sync response to {}", port_identifier, src);
                    }
                } else {
                    warn!(
                        "{}: Clock sync count {} received from {} but no matching participant found",
                        port_identifier, packet.count, src
                    );
                }
            }
            2 => {
                let mut lock = participants.lock().await;
                if let Some(_participant) = lock.get_mut(&packet.sender_ssrc) {
                    let latency_estimate = (packet.timestamps[2] - packet.timestamps[0]) as f32 / 10.0;
                    info!(
                        "{}: Clock sync finalized with {} (latency estimate: {}ms)",
                        port_identifier, src, latency_estimate
                    );
                } else {
                    warn!(
                        "{}: Clock sync count {} received from {} but no matching participant found",
                        port_identifier, packet.count, src
                    );
                }
            }
            _ => {
                error!("{}: Unexpected clock sync count {} from {}", port_identifier, packet.count, src);
            }
        }
    }

    pub async fn send_midi_batch(&self, commands: &[TimedCommand]) -> std::io::Result<()> {
        let ctx_guard = self.session_ctx.lock().await;
        let ctx = ctx_guard
            .as_ref()
            .ok_or(std::io::Error::new(std::io::ErrorKind::NotConnected, "Session not started"))?;
        let lock = ctx.participants.lock().await;
        let participants: Vec<Participant> = lock.values().cloned().collect();
        let mut seq = ctx.sequence_number.lock().await;
        let packet = MidiPacket::new(
            *seq,                                           // Sequence number
            Self::current_timestamp(ctx.start_time) as u32, // Timestamp relative to start_time
            self.ssrc,
            commands,
        );
        *seq = seq.wrapping_add(1); // Increment sequence number, wrapping on overflow
        let packet_bytes = packet.to_bytes(false);
        info!("{}-MIDI: Sending MIDI packet batch to {:?}", ctx.name, participants);
        let midi_socket = ctx.midi_socket.clone();
        for participant in participants {
            midi_socket.send_to(&packet_bytes, participant.midi_port_addr()).await?;
        }
        Ok(())
    }

    pub async fn send_midi(&self, command: &MidiCommand) -> std::io::Result<()> {
        let batch: [TimedCommand; 1] = [TimedCommand::new(None, command.clone())];
        self.send_midi_batch(&batch).await
    }

    fn current_timestamp(start_time: Instant) -> u64 {
        // Modified to take start_time
        (Instant::now() - start_time).as_micros() as u64 / 100
    }

    pub async fn invite_participant(&self, addr: SocketAddr) -> std::io::Result<()> {
        let initiator_token = rand::random::<u32>();
        let invitation = SessionInitiationPacket::new_invitation(initiator_token, self.ssrc, self.name.clone());
        let ctx_guard = self.session_ctx.lock().await;
        let ctx = ctx_guard
            .as_ref()
            .ok_or(std::io::Error::new(std::io::ErrorKind::NotConnected, "Session not started"))?;
        ctx.control_socket.send_to(&invitation.to_bytes(), addr).await?;
        info!("{}-Control: Sent session invitation to {}", self.name, addr);
        // Add to pending_invitations with a placeholder SSRC (0) until we know the remote SSRC from their acknowledgment
        ctx.pending_invitations.lock().await.insert(
            0, // Placeholder SSRC
            PendingInvitation {
                addr,
                token: initiator_token,
                name: String::new(),
            },
        );
        Ok(())
    }

    async fn start_host_clock_sync(ctx: SessionContext) {
        // Use session_ctx to get midi_socket and control_socket
        loop {
            tokio::select! {
                _ = ctx.cancel_token.cancelled() => {
                    debug!("start_host_clock_sync: cancellation requested");
                    break;
                }
                // Sleep for 10 seconds between syncs
                _ = sleep(Duration::from_secs(10)) => {
                    let mut lock = ctx.participants.lock().await;
                    let before_count = lock.len();
                    if before_count == 0 {
                        debug!("No participants to sync with");
                        continue; // No participants to sync with
                    }

                    let stale_participants: Vec<_> = lock
                        .iter()
                        .filter(|(_, p)| p.is_invited_by_us() && Instant::now().duration_since(p.last_clock_sync()) >= Duration::from_secs(30))
                        .map(|(ssrc, p)| (*ssrc, p.clone()))
                        .collect();

                    lock.retain(|ssrc, _| !stale_participants.iter().any(|(stale_ssrc, _)| stale_ssrc == ssrc));

                    for (_ssrc, participant) in stale_participants {
                        let termination_packet = SessionInitiationPacket::new_termination(participant.initiator_token().unwrap(), ctx.ssrc);
                        if let Err(e) = ctx.control_socket.send_to(&termination_packet.to_bytes(), participant.addr()).await {
                            warn!("Failed to send end session packet to {}: {}", participant.addr(), e);
                        } else {
                            info!("Sent end session packet to {}", participant.addr());
                        }
                    }

                    let after_count = lock.len();
                    let removed_count = before_count - after_count;
                    if removed_count > 0 {
                        info!("Removed {} stale participant(s)", removed_count);
                    }

                    let now = Self::current_timestamp(ctx.start_time);
                    let clock_sync = ClockSyncPacket::new(0, [now, 0, 0], ctx.ssrc);
                    let clock_sync_bytes = clock_sync.to_bytes();
                    for p in lock.values_mut() {
                        match ctx.midi_socket.send_to(&clock_sync_bytes, p.midi_port_addr()).await {
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

    async fn handle_acknowledgment(is_control: bool, ack_body: &SessionInitiationPacketBody, ctx: &SessionContext, port_identifier: &str) {
        let src_ssrc = ack_body.sender_ssrc;

        let mut locked_pending_invitations = ctx.pending_invitations.lock().await;
        // If we don't have an entry for this SSRC, but we have a placeholder (0), update the key
        if !locked_pending_invitations.contains_key(&src_ssrc)
            && locked_pending_invitations.contains_key(&0)
            && locked_pending_invitations[&0].token == ack_body.initiator_token
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

                if is_control {
                    debug!(
                        "{}: Matched Acknowledgment from {} invitation. Sending MIDI port invitation.",
                        port_identifier, inv.addr
                    );
                    let response_packet = SessionInitiationPacket::new_invitation(inv.token, ctx.ssrc, inv.name.clone());
                    let response_bytes = response_packet.to_bytes();
                    let midi_addr = SocketAddr::new(inv.addr.ip(), inv.addr.port() + 1);
                    if let Err(e) = ctx.midi_socket.send_to(&response_bytes, midi_addr).await {
                        warn!("{}: Failed to send MIDI port invitation to {}: {}", port_identifier, midi_addr, e);
                    } else {
                        info!("{}: Sent MIDI port invitation to {} with token {}", port_identifier, midi_addr, inv.token);
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
                    debug!(
                        "{}: Matched Acknowledgment from {} for MIDI port invitation. Sending Clock Sync.",
                        port_identifier, inv.addr
                    );
                    let ctrl_addr = SocketAddr::new(inv.addr.ip(), inv.addr.port() - 1);
                    let response_packet = ClockSyncPacket::new(0, [Self::current_timestamp(ctx.start_time), 0, 0], ctx.ssrc);
                    let response_bytes = response_packet.to_bytes();
                    ctx.participants
                        .lock()
                        .await
                        .insert(src_ssrc, Participant::new(ctrl_addr, true, Some(inv.token), inv.name));
                    if let Err(e) = ctx.midi_socket.send_to(&response_bytes, inv.addr).await {
                        warn!("{}: Failed to send clock sync to {}: {}", port_identifier, inv.addr, e);
                    } else {
                        info!("{}: Sent clock sync to {}", port_identifier, inv.addr);
                    }
                }
            } else {
                warn!(
                    "{}: Received Acknowledgment from {} with mismatched token. Expected {}, got {}.",
                    port_identifier, inv.addr, inv.token, ack_body.initiator_token
                );
            }
        } else {
            warn!(
                "{}: Received Acknowledgment from {} but no pending invitation found for this SSRC.",
                port_identifier, src_ssrc
            );
        }
    }

    pub async fn participants(&self) -> Vec<Participant> {
        let ctx_guard = self.session_ctx.lock().await;
        let ctx = ctx_guard.as_ref().expect("Session not started");
        let lock = ctx.participants.lock().await;
        let participants: Vec<Participant> = lock.values().cloned().collect();
        participants
    }
}

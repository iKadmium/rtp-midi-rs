use log::{debug, error, info, trace, warn};
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::UdpSocket;
use tokio::sync::Mutex;
use tokio::task;
use tokio::time::sleep;

#[cfg(feature = "mdns")]
use mdns_sd::{ServiceDaemon, ServiceInfo};

use crate::packets::control_packets::clock_sync_packet::ClockSyncPacket;
use crate::packets::control_packets::control_packet::ControlPacket;
use crate::packets::control_packets::session_initiation_packet::SessionInitiationPacket;
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
    start_time: Instant, // Added start_time
    listeners: Arc<Mutex<ListenerSet>>,
    participants: Arc<Mutex<HashMap<SocketAddr, Participant>>>,
    sequence_number: Arc<Mutex<u16>>,
    midi_socket: Arc<UdpSocket>,
    control_socket: Arc<UdpSocket>,
    pending_invitations: Arc<Mutex<HashMap<SocketAddr, u32>>>, // addr -> initiator_token
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RtpMidiEventType {
    MidiPacket,
}

#[derive(Clone)]
pub struct SessionContext {
    pub name: String,
    pub ssrc: u32,
    pub start_time: Instant,
    pub participants: Arc<Mutex<HashMap<SocketAddr, Participant>>>,
    pub pending_invitations: Arc<Mutex<HashMap<SocketAddr, u32>>>,
    pub midi_socket: Arc<UdpSocket>,
    pub control_socket: Arc<UdpSocket>,
}

impl RtpMidiSession {
    pub async fn new(name: String, ssrc: u32, port: u16) -> std::io::Result<Self> {
        let control_socket = Arc::new(UdpSocket::bind(("0.0.0.0", port)).await?);
        let midi_socket = Arc::new(UdpSocket::bind(("0.0.0.0", port + 1)).await?);
        Ok(Self {
            name,
            ssrc,
            start_time: Instant::now(),
            listeners: Arc::new(Mutex::new(HashMap::new())),
            participants: Arc::new(Mutex::new(HashMap::new())),
            sequence_number: Arc::new(Mutex::new(0)),
            control_socket: control_socket.clone(),
            midi_socket: midi_socket.clone(),
            pending_invitations: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub fn session_context(&self) -> SessionContext {
        SessionContext {
            name: self.name.clone(),
            ssrc: self.ssrc,
            start_time: self.start_time,
            participants: Arc::clone(&self.participants),
            pending_invitations: Arc::clone(&self.pending_invitations),
            midi_socket: Arc::clone(&self.midi_socket),
            control_socket: Arc::clone(&self.control_socket),
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

    pub async fn start<F>(&self, invite_handler: F) -> std::io::Result<()>
    where
        F: Fn(&SessionInitiationPacket, &SocketAddr) -> bool + Send + Sync + 'static,
    {
        // Start periodic stale participant removal
        self.start_host_clock_sync().await;

        // Advertise the service on mDNS
        Self::advertise_mdns(&self.name.clone(), self.control_socket.local_addr()?.port()).expect("Failed to advertise service");

        let listeners_midi = Arc::clone(&self.listeners);
        let session_seq = Arc::clone(&self.sequence_number);
        let invite_handler = Arc::new(invite_handler);

        let ctx = self.session_context();
        let control_task = task::spawn(Self::listen_for_control(ctx.clone(), invite_handler.clone()));
        let midi_task = task::spawn(Self::listen_for_midi(ctx, listeners_midi, session_seq));

        println!("RTP MIDI server starting");

        tokio::select! {
            _ = control_task => {
                debug!("Control task completed");
            },
            _ = midi_task => {
                debug!("MIDI task completed");
            },
        }

        Ok(())
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
        let mut buf = [0; 65535];
        loop {
            match ctx.control_socket.recv_from(&mut buf).await {
                Ok((amt, src)) => {
                    trace!("Control: Received {} bytes from {}", amt, src);
                    match ControlPacket::from_be_bytes(&buf[..amt]) {
                        Ok(packet) => {
                            trace!("Control: Parsed packet: {:?}", packet);
                            match packet {
                                ControlPacket::SessionInitiation(session_initiation_packet) => match &session_initiation_packet {
                                    SessionInitiationPacket::Invitation(invitation) => {
                                        let accept = (invite_handler)(&session_initiation_packet, &src);
                                        if accept {
                                            info!("Control: Accepted session initiation from {}", src);
                                            Self::send_invitation_response(&ctx.control_socket, src, ctx.ssrc, invitation.initiator_token, &ctx.name).await;
                                        } else {
                                            info!("Control: Rejected session initiation from {}", src);
                                            let rejection_packet =
                                                SessionInitiationPacket::new_rejection(invitation.initiator_token, ctx.ssrc, ctx.name.clone());
                                            let _ = ctx.control_socket.send_to(&rejection_packet.to_bytes(), src).await;
                                        }
                                    }
                                    SessionInitiationPacket::Acknowledgment(ack_body) => {
                                        info!("Control: Received session acknowledgment from {} for token {}", src, ack_body.initiator_token);
                                        Self::handle_acknowledgment(true, src, ack_body.initiator_token, &ctx).await;
                                    }
                                    SessionInitiationPacket::Rejection(_) => {
                                        info!("Control: Received session rejection from {}", src);
                                    }
                                    SessionInitiationPacket::Termination(_) => {
                                        info!("Control: Received session termination from {}", src);
                                        Self::handle_end_session(src, &ctx.participants);
                                    }
                                },
                                _ => {
                                    warn!("Control: Unhandled control packet: {:?}", packet);
                                }
                            }
                        }
                        Err(e) => {
                            warn!("Control: {} from {}", e, src);
                        }
                    }
                }
                Err(e) => {
                    error!("Control: Error receiving data: {}", e);
                    break;
                }
            }
        }
    }

    async fn listen_for_midi(ctx: SessionContext, listeners: Arc<Mutex<ListenerSet>>, session_seq: Arc<Mutex<u16>>) {
        let mut buf = [0; 65535];
        loop {
            match ctx.midi_socket.recv_from(&mut buf).await {
                Ok((amt, src)) => {
                    trace!("MIDI: Received {} bytes from {}", amt, src);
                    match RtpMidiPacket::parse(&buf[..amt]) {
                        Ok(packet) => {
                            trace!("MIDI: Parsed RTP MIDI packet: {:?}", packet);
                            match packet {
                                RtpMidiPacket::Control(control_packet) => match control_packet {
                                    ControlPacket::SessionInitiation(session_initiation_packet) => match session_initiation_packet {
                                        SessionInitiationPacket::Invitation(invitation) => {
                                            info!("MIDI: Received session invitation from {}", src);
                                            Self::send_invitation_response(&ctx.midi_socket, src, ctx.ssrc, invitation.initiator_token, &ctx.name).await;
                                        }
                                        SessionInitiationPacket::Acknowledgment(ack_body) => {
                                            info!("MIDI: Received session acknowledgment from {} for token {}", src, ack_body.initiator_token);
                                            Self::handle_acknowledgment(false, src, ack_body.initiator_token, &ctx).await;
                                        }
                                        _ => {
                                            warn!("MIDI: Unhandled session initiation packet {:?}", session_initiation_packet);
                                        }
                                    },
                                    ControlPacket::ClockSync(clock_sync_packet) => {
                                        debug!("MIDI: Received clock sync from {}", src);
                                        Self::handle_clock_sync(
                                            &ctx.midi_socket,
                                            clock_sync_packet,
                                            src,
                                            ctx.ssrc,
                                            ctx.participants.clone(),
                                            ctx.start_time,
                                            ctx.pending_invitations.clone(),
                                        )
                                        .await;
                                    }
                                },
                                RtpMidiPacket::Midi(midi_packet) => {
                                    debug!("MIDI: Parsed MIDI packet: {:#?}", midi_packet);
                                    let mut seq = session_seq.lock().await;
                                    *seq = midi_packet.sequence_number().wrapping_add(1);
                                    if let Some(callback) = listeners.lock().await.get(&RtpMidiEventType::MidiPacket) {
                                        callback(midi_packet);
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            error!("MIDI: Failed to parse RTP MIDI packet: {}", e);
                        }
                    }
                }
                Err(e) => {
                    error!("MIDI: Error receiving data: {}", e);
                    break;
                }
            }
        }
    }

    async fn send_invitation_response(socket: &UdpSocket, src: SocketAddr, sender_ssrc: u32, initiator_token: u32, name: &str) {
        let response_packet = SessionInitiationPacket::new_acknowledgment(initiator_token, sender_ssrc, name.to_string());

        if let Err(e) = socket.send_to(&response_packet.to_bytes(), src).await {
            error!("{}: Failed to send invitation response to {}: {}", socket.local_addr().unwrap().port(), src, e);
        } else {
            info!("{}: Sent invitation response to {}", socket.local_addr().unwrap().port(), src);
        }
    }

    fn handle_end_session(control_port_addr: SocketAddr, participants: &Arc<Mutex<HashMap<SocketAddr, Participant>>>) {
        // Update `handle_end_session` to use the control port address when removing participants
        info!("Control: Ending session with {}", control_port_addr);
        let participants = Arc::clone(participants);
        tokio::spawn(async move {
            let mut lock = participants.lock().await;
            lock.remove(&control_port_addr);
        });
    }

    async fn handle_clock_sync(
        socket: &UdpSocket,
        packet: ClockSyncPacket,
        src: SocketAddr,
        ssrc: u32,
        participants: Arc<Mutex<HashMap<SocketAddr, Participant>>>,
        start_time: Instant, // Added start_time parameter
        pending_invitations: Arc<Mutex<HashMap<SocketAddr, u32>>>,
    ) {
        match packet.count {
            0 => {
                // Respond with count = 1
                let timestamp2 = Self::current_timestamp(start_time); // Use start_time
                let response_packet = ClockSyncPacket::new(1, [packet.timestamps[0], timestamp2, 0], ssrc);

                if let Err(e) = socket.send_to(&response_packet.to_bytes(), src).await {
                    error!("MIDI: Failed to send clock sync response to {}: {}", src, e);
                } else {
                    debug!("MIDI: Sent clock sync response to {}", src);
                }
            }
            1 => {
                // Remove pending invitation and add as participant if needed
                let mut invitations = pending_invitations.lock().await;
                let token = invitations.get(&src).cloned();
                invitations.remove(&src);
                drop(invitations);

                let control_port_addr = SocketAddr::new(src.ip(), src.port() - 1); // Use control port address
                let mut lock = participants.lock().await;
                // Check if the participant is already in the list
                let entry = lock.entry(control_port_addr);
                if let Entry::Occupied(mut entry) = entry {
                    entry.get_mut().received_clock_sync();
                    debug!("MIDI: Updated clock sync for existing participant {}", control_port_addr);
                } else if let Entry::Vacant(entry) = entry {
                    if token.is_none() {
                        error!("MIDI: Received clock sync from {} without a valid token", src);
                        return;
                    }
                    info!("Added {} as participant after clock sync", control_port_addr);
                    entry.insert(Participant::new(control_port_addr, true, token)); // Mark as not invited by us
                }

                // Respond with count = 2
                let timestamp3 = Self::current_timestamp(start_time); // Use start_time
                let response_packet = ClockSyncPacket::new(2, [packet.timestamps[0], packet.timestamps[1], timestamp3], ssrc);

                if let Err(e) = socket.send_to(&response_packet.to_bytes(), src).await {
                    error!("MIDI: Failed to send clock sync count 2 to {}: {}", src, e);
                } else {
                    debug!("MIDI: Sent clock sync count 2 to {}", src);
                }
            }
            2 => {
                // Finalize clock sync
                info!("MIDI: Clock sync finalized with {}", src);
                let latency_estimate = (packet.timestamps[2] - packet.timestamps[0]) as f32 / 10.0;
                info!("MIDI: Clock sync latency estimate: {}ms", latency_estimate);
                let mut lock = participants.lock().await;
                let control_port_addr = SocketAddr::new(src.ip(), src.port() - 1); // Use control port address
                lock.insert(control_port_addr, Participant::new(control_port_addr, false, None)); // Mark as not invited by us
            }
            _ => {
                error!("MIDI: Unexpected clock sync count {} from {}", packet.count, src);
            }
        }
    }

    pub async fn send_midi_batch(&self, commands: &[TimedCommand]) -> std::io::Result<()> {
        let lock = self.participants.lock().await;
        let participants: Vec<Participant> = lock.values().cloned().collect();
        let mut seq = self.sequence_number.lock().await;
        let packet = MidiPacket::new(
            *seq,                                            // Sequence number
            Self::current_timestamp(self.start_time) as u32, // Timestamp relative to start_time
            self.ssrc,
            commands,
        );
        *seq = seq.wrapping_add(1); // Increment sequence number, wrapping on overflow
        let packet_bytes = packet.to_bytes(false);
        info!("Sending MIDI packet batch to {:?}", participants);
        for participant in participants {
            self.midi_socket.send_to(&packet_bytes, participant.midi_port_addr()).await?; // Use self.midi_socket directly
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
        // 1. Generate token and send invitation
        let initiator_token = rand::random::<u32>();
        let invitation = SessionInitiationPacket::new_invitation(initiator_token, self.ssrc, self.name.clone());
        self.control_socket.send_to(&invitation.to_bytes(), addr).await?;
        info!("Sent session invitation to {}", addr);

        // 2. Record pending invitation
        self.pending_invitations.lock().await.insert(addr, initiator_token);
        Ok(())
    }

    pub async fn start_host_clock_sync(&self) {
        let midi_socket = self.midi_socket.clone();
        let control_socket = self.control_socket.clone();
        let participants = self.participants.clone();
        let ssrc = self.ssrc;
        let start_time = self.start_time;
        tokio::spawn(async move {
            loop {
                sleep(Duration::from_secs(10)).await;

                let mut lock = participants.lock().await;
                let before_count = lock.len();
                if before_count == 0 {
                    debug!("No participants to sync with");
                    continue; // No participants to sync with
                }

                let stale_participants: Vec<_> = lock
                    .iter()
                    .filter(|(_, p)| p.is_invited_by_us() && Instant::now().duration_since(p.last_clock_sync()) >= Duration::from_secs(30))
                    .map(|(addr, p)| (*addr, p.clone()))
                    .collect();

                lock.retain(|addr, _| !stale_participants.iter().any(|(stale_addr, _)| stale_addr == addr));

                for (addr, participant) in stale_participants {
                    let termination_packet = SessionInitiationPacket::new_termination(participant.initiator_token().unwrap(), ssrc);
                    if let Err(e) = control_socket.send_to(&termination_packet.to_bytes(), addr).await {
                        warn!("Failed to send end session packet to {}: {}", addr, e);
                    } else {
                        info!("Sent end session packet to {}", addr);
                    }
                }

                let after_count = lock.len();
                let removed_count = before_count - after_count;
                if removed_count > 0 {
                    info!("Removed {} stale participant(s)", removed_count);
                }

                let now = Self::current_timestamp(start_time);
                let clock_sync = ClockSyncPacket::new(0, [now, 0, 0], ssrc);
                let clock_sync_bytes = clock_sync.to_bytes();
                for p in lock.values_mut() {
                    match midi_socket.send_to(&clock_sync_bytes, p.midi_port_addr()).await {
                        Ok(_) => {
                            debug!("Sent clock sync to {}", p.midi_port_addr());
                        }
                        Err(e) => {
                            warn!("Failed to send clock sync to {}: {}", p.midi_port_addr(), e);
                        }
                    }
                }
            }
        });
    }

    async fn handle_acknowledgment(is_control: bool, src: SocketAddr, ack_token: u32, ctx: &SessionContext) {
        let label = if is_control { "Control" } else { "MIDI" };
        let mut locked_pending_invitations = ctx.pending_invitations.lock().await;
        if let Some(expected_token) = locked_pending_invitations.get(&src).cloned() {
            if expected_token == ack_token {
                if is_control {
                    locked_pending_invitations.remove(&src);
                }
                drop(locked_pending_invitations);
                let response_bytes = match is_control {
                    true => SessionInitiationPacket::new_invitation(expected_token, ctx.ssrc, ctx.name.clone()).to_bytes(),
                    false => ClockSyncPacket::new(0, [Self::current_timestamp(ctx.start_time), 0, 0], ctx.ssrc).to_bytes(),
                };
                let midi_addr = match is_control {
                    true => SocketAddr::new(src.ip(), src.port() + 1),
                    false => src,
                };
                if is_control {
                    debug!("Control: Matched Acknowledgment from {} invitation. Sending MIDI port invitation.", src);
                    if let Err(e) = ctx.midi_socket.send_to(&response_bytes, midi_addr).await {
                        warn!("Control: Failed to send MIDI port invitation to {}: {}", midi_addr, e);
                    } else {
                        info!("Control: Sent MIDI port invitation to {} with token {}", midi_addr, expected_token);
                        ctx.pending_invitations.lock().await.insert(midi_addr, expected_token);
                    }
                } else {
                    debug!("MIDI: Matched Acknowledgment from {} for MIDI port invitation. Sending Clock Sync.", src);
                    if let Err(e) = ctx.midi_socket.send_to(&response_bytes, midi_addr).await {
                        warn!("MIDI: Failed to send clock sync to {}: {}", src, e);
                    } else {
                        info!("MIDI: Sent clock sync to {}", src);
                    }
                }
            } else {
                warn!(
                    "{}: Received Acknowledgment from {} with mismatched token. Expected {}, got {}.",
                    label, src, expected_token, ack_token
                );
            }
        } else {
            warn!(
                "{}: Received Acknowledgment from {} but no pending invitation found for this address.",
                label, src
            );
        }
    }
}

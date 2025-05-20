use log::{debug, error, info, trace, warn};
use mdns_sd::{ServiceDaemon, ServiceInfo};
use std::collections::HashMap;
use std::io::Cursor; // Added import for Cursor
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::UdpSocket;
use tokio::sync::Mutex;
use tokio::task;

use crate::packet::control_packets::clock_sync_packet::ClockSyncPacket;
use crate::packet::control_packets::control_packet::ControlPacket;
use crate::packet::control_packets::session_initiation_packet::SessionInitiationPacket;
use crate::packet::midi_packets::midi_command::MidiCommand;
use crate::packet::midi_packets::midi_packet::MidiPacket;
use crate::packet::midi_packets::midi_timed_command::TimedCommand;
use crate::packet::packet::RtpMidiPacket;
use crate::participant::Participant;

pub struct RtpMidiSession {
    name: String,
    ssrc: u32,
    start_time: Instant, // Added start_time
    listeners: Arc<Mutex<HashMap<RtpMidiEventType, Box<dyn Fn(MidiPacket) + Send>>>>,
    participants: Arc<Mutex<HashMap<SocketAddr, Participant>>>,
    sequence_number: Arc<Mutex<u16>>,
    midi_socket: Arc<UdpSocket>,
    control_socket: Arc<UdpSocket>, // Changed from Arc<Mutex<UdpSocket>>
    pending_invitations: Arc<Mutex<HashMap<SocketAddr, u32>>>, // addr -> initiator_token
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RtpMidiEventType {
    MidiPacket,
}

impl RtpMidiSession {
    pub async fn new(name: String, ssrc: u32, port: u16) -> std::io::Result<Self> {
        Ok(Self {
            name,
            ssrc,
            start_time: Instant::now(), // Initialize start_time
            listeners: Arc::new(Mutex::new(HashMap::new())),
            participants: Arc::new(Mutex::new(HashMap::new())),
            sequence_number: Arc::new(Mutex::new(0)),
            control_socket: Arc::new(UdpSocket::bind(("0.0.0.0", port)).await?), // Removed Mutex::new()
            midi_socket: Arc::new(UdpSocket::bind(("0.0.0.0", port + 1)).await?),
            pending_invitations: Arc::new(Mutex::new(HashMap::new())), // Initialize pending_invitations
        })
    }

    pub async fn add_listener<F>(&self, event_type: RtpMidiEventType, callback: F)
    where
        F: Fn(MidiPacket) + Send + 'static,
    {
        let mut listeners = self.listeners.lock().await;
        listeners.insert(event_type, Box::new(callback));
    }

    pub async fn start(&self) -> std::io::Result<()> {
        // Start periodic stale participant removal
        self.start_stale_removal_task().await;
        self.start_host_clock_sync().await;

        // Advertise the service on mDNS
        Self::advertise_service(&self.name.clone(), self.control_socket.local_addr()?.port())
            .expect("Failed to advertise service");

        let session_name = self.name.clone();
        let listeners_midi = Arc::clone(&self.listeners);
        let participants_clone_control = Arc::clone(&self.participants); // Renamed for clarity
        let participants_clone_midi = Arc::clone(&self.participants); // Renamed for clarity
        let session_seq = Arc::clone(&self.sequence_number);
        let start_time = self.start_time; // Capture start_time

        let control_task = task::spawn(Self::listen_for_control(
            self.control_socket.clone(),
            session_name.clone(),
            self.ssrc,
            participants_clone_control,
            self.pending_invitations.clone(),
            self.midi_socket.clone(),
        ));

        let midi_task = task::spawn(Self::listen_for_midi(
            self.midi_socket.clone(),
            session_name,
            self.ssrc,
            listeners_midi,
            participants_clone_midi,
            session_seq,
            start_time, // Pass start_time
            self.pending_invitations.clone(),
        ));

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

    fn advertise_service(instance_name: &str, port: u16) -> Result<(), mdns_sd::Error> {
        let mdns = ServiceDaemon::new()?;
        let service_type = "_apple-midi._udp.local.";
        let ip = local_ip_address::local_ip()
            .expect("Failed to get local IP address")
            .to_string();

        let raw_hostname = hostname::get()
            .expect("Failed to get hostname")
            .to_string_lossy()
            .to_string();
        let hostname = format!("{}.local.", raw_hostname);
        let props = [("apple-midi", "RTP-MIDI")];
        let service =
            ServiceInfo::new(service_type, instance_name, &hostname, ip, port, &props[..])?;
        mdns.register(service)?;

        //sleep(Duration::from_secs(60));

        Ok(())
    }

    async fn listen_for_control(
        socket: Arc<UdpSocket>, // Changed from Arc<Mutex<UdpSocket>>
        name: String,
        ssrc: u32,
        participants: Arc<Mutex<HashMap<SocketAddr, Participant>>>,
        pending_invitations: Arc<Mutex<HashMap<SocketAddr, u32>>>,
        midi_socket: Arc<UdpSocket>,
    ) {
        let mut buf = [0; 65535];
        loop {
            // Removed: let socket = socket.lock().await;
            match socket.recv_from(&mut buf).await {
                // Use socket directly
                Ok((amt, src)) => {
                    trace!("Control: Received {} bytes from {}", amt, src);
                    match ControlPacket::from_be_bytes(&buf[..amt]) {
                        Ok(packet) => {
                            trace!("Control: Parsed packet: {:?}", packet);
                            match packet {
                                ControlPacket::SessionInitiation(session_initiation_packet) => {
                                    match session_initiation_packet {
                                        SessionInitiationPacket::Invitation(invitaiton) => {
                                            info!(
                                                "Control: Received session initiation from {}",
                                                src
                                            );
                                            Self::send_invitation_response(
                                                &socket, // Pass &socket (Arc derefs to UdpSocket)
                                                src,
                                                ssrc,
                                                invitaiton.initiator_token,
                                                &name,
                                            )
                                            .await;
                                        }
                                        SessionInitiationPacket::Acknowledgment(ack_body) => {
                                            info!(
                                                "Control: Received session acknowledgment from {} for token {}",
                                                src, ack_body.initiator_token
                                            );
                                            let mut locked_pending_invitations =
                                                pending_invitations.lock().await;
                                            if let Some(expected_token) =
                                                locked_pending_invitations.get(&src).cloned()
                                            {
                                                if expected_token == ack_body.initiator_token {
                                                    // Control port ACK matches our pending invitation.
                                                    locked_pending_invitations.remove(&src); // Clean up control port invitation
                                                    drop(locked_pending_invitations); // Release lock before await

                                                    info!(
                                                        "Control: Matched Acknowledgment from {} for control port invitation. Sending MIDI port invitation.",
                                                        src
                                                    );

                                                    // Now, send an Invitation on the MIDI port
                                                    let peer_midi_addr =
                                                        SocketAddr::new(src.ip(), src.port() + 1);
                                                    let midi_initiator_token =
                                                        rand::random::<u32>();

                                                    let midi_invitation_packet =
                                                        SessionInitiationPacket::new_invitation(
                                                            midi_initiator_token,
                                                            ssrc,         // Our SSRC
                                                            name.clone(), // Our session name
                                                        );
                                                    let mut midi_invitation_buf =
                                                        Vec::with_capacity(
                                                            midi_invitation_packet.size(),
                                                        );
                                                    midi_invitation_packet
                                                        .write(&mut midi_invitation_buf)
                                                        .unwrap();

                                                    if let Err(e) = midi_socket
                                                        .send_to(
                                                            &midi_invitation_buf,
                                                            peer_midi_addr,
                                                        )
                                                        .await
                                                    {
                                                        warn!(
                                                            "Control: Failed to send MIDI port invitation to {}: {}",
                                                            peer_midi_addr, e
                                                        );
                                                    } else {
                                                        info!(
                                                            "Control: Sent MIDI port invitation to {} with token {}",
                                                            peer_midi_addr, midi_initiator_token
                                                        );
                                                        // Record this new pending invitation, expecting an ACK on our MIDI port from peer_midi_addr
                                                        pending_invitations.lock().await.insert(
                                                            peer_midi_addr,
                                                            midi_initiator_token,
                                                        );
                                                    }
                                                } else {
                                                    warn!(
                                                        "Control: Received Acknowledgment from {} with mismatched token. Expected {}, got {}.",
                                                        src,
                                                        expected_token,
                                                        ack_body.initiator_token
                                                    );
                                                }
                                            } else {
                                                warn!(
                                                    "Control: Received Acknowledgment from {} but no pending invitation found for this address.",
                                                    src
                                                );
                                            }
                                        }
                                        SessionInitiationPacket::Rejection(_) => {
                                            info!(
                                                "Control: Received session rejection from {}",
                                                src
                                            );
                                        }
                                        SessionInitiationPacket::Termination(_) => {
                                            info!(
                                                "Control: Received session termination from {}",
                                                src
                                            );
                                            Self::handle_end_session(src, &participants);
                                        }
                                    }
                                }
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

    async fn listen_for_midi(
        socket: Arc<UdpSocket>, // Changed from Arc<Mutex<UdpSocket>>
        server_name: String,
        ssrc: u32,
        listeners: Arc<Mutex<HashMap<RtpMidiEventType, Box<dyn Fn(MidiPacket) + Send>>>>,
        participants: Arc<Mutex<HashMap<SocketAddr, Participant>>>,
        session_seq: Arc<Mutex<u16>>,
        start_time: Instant, // Added start_time parameter
        pending_invitations: Arc<Mutex<HashMap<SocketAddr, u32>>>,
    ) {
        let mut buf = [0; 65535];
        loop {
            match socket.recv_from(&mut buf).await {
                // Use socket directly
                Ok((amt, src)) => {
                    trace!("MIDI: Received {} bytes from {}", amt, src);
                    match RtpMidiPacket::parse(&buf[..amt]) {
                        Ok(packet) => {
                            trace!("MIDI: Parsed RTP MIDI packet: {:?}", packet);
                            match packet {
                                RtpMidiPacket::Control(control_packet) => match control_packet {
                                    ControlPacket::SessionInitiation(session_initiation_packet) => {
                                        match session_initiation_packet {
                                            SessionInitiationPacket::Invitation(invitation) => {
                                                info!(
                                                    "MIDI: Received session invitation from {}",
                                                    src
                                                );
                                                Self::send_invitation_response(
                                                    &socket, // Pass &socket (Arc derefs to UdpSocket)
                                                    src,
                                                    ssrc,
                                                    invitation.initiator_token,
                                                    &server_name,
                                                )
                                                .await;
                                            }
                                            SessionInitiationPacket::Acknowledgment(ack_body) => {
                                                info!(
                                                    "MIDI: Received session acknowledgment from {} for token {}",
                                                    src, ack_body.initiator_token
                                                );
                                                let locked_pending_invitations =
                                                    pending_invitations.lock().await;
                                                if let Some(expected_token) =
                                                    locked_pending_invitations.get(&src).cloned()
                                                {
                                                    if expected_token == ack_body.initiator_token {
                                                        // MIDI port ACK matches our pending invitation.
                                                        drop(locked_pending_invitations); // Release lock before await

                                                        info!(
                                                            "MIDI: Matched Acknowledgment from {} for MIDI port invitation. Sending Clock Sync.",
                                                            src
                                                        );
                                                        let timestamp =
                                                            Self::current_timestamp(start_time);
                                                        let clock_sync_packet =
                                                            ClockSyncPacket::new(
                                                                0,
                                                                [timestamp, 0, 0],
                                                                ssrc,
                                                            );
                                                        let mut sync_buf =
                                                            vec![0u8; ClockSyncPacket::SIZE];
                                                        let mut cursor = Cursor::new(&mut sync_buf);
                                                        clock_sync_packet
                                                            .write(&mut cursor)
                                                            .unwrap();
                                                        if let Err(e) =
                                                            socket.send_to(&sync_buf, src).await
                                                        {
                                                            warn!(
                                                                "MIDI: Failed to send clock sync to {}: {}",
                                                                src, e
                                                            );
                                                        } else {
                                                            info!(
                                                                "MIDI: Sent clock sync to {}",
                                                                src
                                                            );
                                                        }
                                                    } else {
                                                        warn!(
                                                            "MIDI: Received Acknowledgment from {} with mismatched token. Expected {}, got {}.",
                                                            src,
                                                            expected_token,
                                                            ack_body.initiator_token
                                                        );
                                                    }
                                                } else {
                                                    warn!(
                                                        "MIDI: Received Acknowledgment from {} but no pending invitation found for this address.",
                                                        src
                                                    );
                                                }
                                            }
                                            _ => {
                                                warn!(
                                                    "MIDI: Unhandled session initiation packet {:?}",
                                                    session_initiation_packet
                                                );
                                            }
                                        }
                                    }
                                    ControlPacket::ClockSync(clock_sync_packet) => {
                                        debug!("MIDI: Received clock sync from {}", src);
                                        // Always delegate to handle_clock_sync for further protocol
                                        Self::handle_clock_sync(
                                            &socket, // Pass &socket (Arc derefs to UdpSocket)
                                            clock_sync_packet,
                                            src,
                                            ssrc,
                                            participants.clone(),
                                            start_time, // Pass start_time
                                            pending_invitations.clone(),
                                        )
                                        .await;
                                    }
                                },
                                RtpMidiPacket::Midi(midi_packet) => {
                                    debug!("MIDI: Parsed MIDI packet: {:#?}", midi_packet);
                                    // Update sequence number on receive
                                    let mut seq = session_seq.lock().await;
                                    *seq = midi_packet.sequence_number().wrapping_add(1);
                                    if let Some(callback) =
                                        listeners.lock().await.get(&RtpMidiEventType::MidiPacket)
                                    {
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

    async fn send_invitation_response(
        socket: &UdpSocket,
        src: std::net::SocketAddr,
        sender_ssrc: u32,
        initiator_token: u32,
        name: &str,
    ) {
        let response_packet = SessionInitiationPacket::new_acknowledgment(
            initiator_token,
            sender_ssrc,
            name.to_string(),
        );

        let mut response_bytes = Vec::with_capacity(response_packet.size());
        response_packet.write(&mut response_bytes).unwrap();

        if let Err(e) = socket.send_to(&response_bytes, src).await {
            error!(
                "{}: Failed to send invitation response to {}: {}",
                socket.local_addr().unwrap().port(),
                src,
                e
            );
        } else {
            info!(
                "{}: Sent invitation response to {}",
                socket.local_addr().unwrap().port(),
                src
            );
        }
    }

    fn handle_end_session(
        src: std::net::SocketAddr,
        participants: &Arc<Mutex<HashMap<SocketAddr, Participant>>>,
    ) {
        // Update `handle_end_session` to use the control port address when removing participants
        let control_port_addr = SocketAddr::new(src.ip(), src.port() - 1); // Use control port address
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
        src: std::net::SocketAddr,
        ssrc: u32,
        participants: Arc<Mutex<HashMap<SocketAddr, Participant>>>,
        start_time: Instant, // Added start_time parameter
        pending_invitations: Arc<Mutex<HashMap<SocketAddr, u32>>>,
    ) {
        match packet.count {
            0 => {
                // Respond with count = 1
                let timestamp2 = Self::current_timestamp(start_time); // Use start_time
                let response_packet =
                    ClockSyncPacket::new(1, [packet.timestamps[0], timestamp2, 0], ssrc);

                let mut response_bytes = vec![0u8; ClockSyncPacket::SIZE];
                let mut cursor = Cursor::new(&mut response_bytes);
                response_packet.write(&mut cursor).unwrap();

                if let Err(e) = socket.send_to(&response_bytes, src).await {
                    error!("MIDI: Failed to send clock sync response to {}: {}", src, e);
                } else {
                    debug!("MIDI: Sent clock sync response to {}", src);
                }
            }
            1 => {
                // Remove pending invitation and add as participant if needed
                let control_port_addr = SocketAddr::new(src.ip(), src.port() - 1); // Use control port address
                let mut lock = participants.lock().await;
                lock.insert(
                    control_port_addr,
                    Participant::new(control_port_addr, true), // Mark as invited by us
                );
                info!(
                    "Added {} as participant after clock sync",
                    control_port_addr
                );

                // Respond with count = 2
                let timestamp3 = Self::current_timestamp(start_time); // Use start_time
                let response_packet = ClockSyncPacket::new(
                    2,
                    [packet.timestamps[0], packet.timestamps[1], timestamp3],
                    ssrc,
                );

                let mut response_bytes = vec![0u8; ClockSyncPacket::SIZE];
                let mut cursor = Cursor::new(&mut response_bytes);
                response_packet.write(&mut cursor).unwrap();

                if let Err(e) = socket.send_to(&response_bytes, src).await {
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
                lock.insert(
                    control_port_addr,
                    Participant::new(control_port_addr, false),
                ); // Mark as not invited by us
            }
            _ => {
                error!(
                    "MIDI: Unexpected clock sync count {} from {}",
                    packet.count, src
                );
            }
        }
    }

    pub async fn remove_stale(&self) {
        let mut participants = self.participants.lock().await;
        let now = Instant::now();
        let before = participants.len();
        participants
            .retain(|_, p| now.duration_since(p.last_clock_sync()) < Duration::from_secs(30));
        let after = participants.len();
        if before != after {
            info!("Removed {} stale participant(s)", before - after);
        }
    }

    pub async fn all_participants(&self) -> Vec<Participant> {
        let participants = self.participants.lock().await;
        participants.values().cloned().collect()
    }

    pub async fn send_midi_batch(&self, commands: &[TimedCommand]) -> std::io::Result<()> {
        let participants = self.all_participants().await;
        let mut seq = self.sequence_number.lock().await;
        let packet = MidiPacket::new(
            *seq,                                            // Sequence number
            Self::current_timestamp(self.start_time) as u32, // Timestamp relative to start_time
            self.ssrc,
            commands,
        );
        *seq = seq.wrapping_add(1); // Increment sequence number, wrapping on overflow
        let mut data = vec![0u8; packet.size(false)]; // Allocate buffer for packet
        let mut cursor = Cursor::new(&mut data);
        packet.write(&mut cursor, false)?;

        info!("Sending MIDI packet batch to {:?}", participants);
        for participant in participants {
            self.midi_socket
                .send_to(&data, participant.midi_port_addr())
                .await?; // Use self.midi_socket directly
        }
        Ok(())
    }

    pub async fn send_midi(&self, command: &MidiCommand) -> std::io::Result<()> {
        let participants = self.all_participants().await;
        let mut seq = self.sequence_number.lock().await;
        let commands = vec![TimedCommand::new(None, command.clone())];
        let packet = MidiPacket::new(
            *seq,                                            // Sequence number
            Self::current_timestamp(self.start_time) as u32, // Timestamp relative to start_time
            self.ssrc,
            &commands,
        );
        *seq = seq.wrapping_add(1); // Increment sequence number, wrapping on overflow
        let mut data = vec![0u8; packet.size(false)]; // Allocate buffer for packet
        let mut cursor = Cursor::new(&mut data);
        packet.write(&mut cursor, false)?;

        info!("Sending MIDI packet to {:?}", participants);
        for participant in participants {
            self.midi_socket
                .send_to(&data, participant.midi_port_addr())
                .await?; // Use self.midi_socket directly
        }
        Ok(())
    }

    fn current_timestamp(start_time: Instant) -> u64 {
        // Modified to take start_time
        (Instant::now() - start_time).as_micros() as u64 / 100
    }

    pub async fn invite_participant(&self, addr: SocketAddr) -> std::io::Result<()> {
        // 1. Generate token and send invitation
        let initiator_token = rand::random::<u32>();
        let invitation =
            SessionInitiationPacket::new_invitation(initiator_token, self.ssrc, self.name.clone());
        let mut buf = Vec::with_capacity(invitation.size());
        invitation.write(&mut buf).unwrap();
        self.control_socket.send_to(&buf, addr).await?;
        info!("Sent session invitation to {}", addr);

        // 2. Record pending invitation
        self.pending_invitations
            .lock()
            .await
            .insert(addr, initiator_token);
        Ok(())
    }

    pub async fn start_host_clock_sync(&self) {
        use tokio::time::{Duration, sleep};
        let midi_socket = self.midi_socket.clone();
        let participants = self.participants.clone();
        let ssrc = self.ssrc;
        let start_time = self.start_time;
        tokio::spawn(async move {
            loop {
                sleep(Duration::from_secs(10)).await;
                let now = Self::current_timestamp(start_time);
                let clock_sync = ClockSyncPacket::new(0, [now, 0, 0], ssrc);
                let mut sync_buf = vec![0u8; ClockSyncPacket::SIZE];
                let mut cursor = Cursor::new(&mut sync_buf);
                clock_sync.write(&mut cursor).unwrap();
                let addrs: Vec<_> = {
                    let lock = participants.lock().await;
                    lock.values()
                        .filter(|p| p.is_invited_by_us()) // Only include participants invited by us
                        .map(|p| p.midi_port_addr())
                        .collect()
                };
                let participant_count = addrs.len();
                for addr in addrs {
                    let _ = midi_socket.send_to(&sync_buf, addr).await;
                }
                debug!(
                    "Host: Sent periodic clock sync to {} participants",
                    participant_count
                );
            }
        });
    }

    pub async fn start_stale_removal_task(&self) {
        let session = self.clone_for_task();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(10)).await;
                session.remove_stale().await;
            }
        });
    }

    fn clone_for_task(&self) -> RtpMidiSession {
        RtpMidiSession {
            name: self.name.clone(),
            ssrc: self.ssrc,
            start_time: self.start_time,
            listeners: Arc::clone(&self.listeners),
            participants: Arc::clone(&self.participants),
            sequence_number: Arc::clone(&self.sequence_number),
            midi_socket: Arc::clone(&self.midi_socket),
            control_socket: Arc::clone(&self.control_socket),
            pending_invitations: Arc::clone(&self.pending_invitations),
        }
    }
}

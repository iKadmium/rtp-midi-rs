use log::{debug, error, info, trace, warn};
use mdns_sd::{ServiceDaemon, ServiceInfo};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::UdpSocket;
use tokio::sync::Mutex;
use tokio::task;

use crate::packet::clock_sync_packet::ClockSyncPacket;
use crate::packet::control_packet::ControlPacket;
use crate::packet::midi_packet::midi_packet::MidiPacket;
use crate::packet::midi_packet::midi_timed_command::TimedCommand;
use crate::packet::packet::RtpMidiPacket;
use crate::packet::session_initiation_packet::SessionInitiationPacket;

pub struct RtpMidiSession {
    name: String,
    ssrc: u32,
    listeners: Arc<Mutex<HashMap<String, Box<dyn Fn(MidiPacket) + Send>>>>,
    participants: Arc<Mutex<HashMap<SocketAddr, Participant>>>,
    sequence_number: Arc<Mutex<u16>>,
    midi_socket: Arc<Mutex<UdpSocket>>,
    control_socket: Arc<Mutex<UdpSocket>>,
}

impl RtpMidiSession {
    pub async fn new(name: String, ssrc: u32, port: u16) -> std::io::Result<Self> {
        Ok(Self {
            name,
            ssrc,
            listeners: Arc::new(Mutex::new(HashMap::new())),
            participants: Arc::new(Mutex::new(HashMap::new())),
            sequence_number: Arc::new(Mutex::new(0)),
            control_socket: Arc::new(Mutex::new(UdpSocket::bind(("0.0.0.0", port)).await?)),
            midi_socket: Arc::new(Mutex::new(UdpSocket::bind(("0.0.0.0", port + 1)).await?)),
        })
    }

    pub async fn add_listener<F>(&self, event_name: String, callback: F)
    where
        F: Fn(MidiPacket) + Send + 'static,
    {
        let mut listeners = self.listeners.lock().await;
        listeners.insert(event_name, Box::new(callback));
    }

    pub async fn start(&self) -> std::io::Result<()> {
        // Advertise the service on mDNS
        // Self::advertise_service(&self.name.clone(), midi_port)
        //     .expect("Failed to advertise service");

        let session_name = self.name.clone();
        let listeners_midi = Arc::clone(&self.listeners);
        let participants = Arc::clone(&self.participants);
        let session_seq = Arc::clone(&self.sequence_number);

        let control_task = task::spawn(Self::listen_for_control(
            self.control_socket.clone(),
            session_name.clone(),
            self.ssrc,
            participants.clone(),
        ));

        let midi_task = task::spawn(Self::listen_for_midi(
            self.midi_socket.clone(),
            session_name,
            self.ssrc,
            listeners_midi,
            participants,
            session_seq,
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
        socket: Arc<Mutex<UdpSocket>>,
        name: String,
        ssrc: u32,
        participants: Arc<Mutex<HashMap<SocketAddr, Participant>>>,
    ) {
        let socket = socket.lock().await;
        let mut buf = [0; 65535];
        loop {
            match socket.recv_from(&mut buf).await {
                Ok((amt, src)) => {
                    trace!("Control: Received {} bytes from {}", amt, src);
                    match ControlPacket::parse(&buf[..amt]) {
                        Ok(packet) => {
                            trace!("Control: Parsed packet: {:?}", packet);
                            match packet {
                                ControlPacket::SessionInitiation(session_initiation_packet) => {
                                    info!("Control: Received session initiation from {}", src);
                                    Self::send_invitation_response(
                                        &socket,
                                        src,
                                        ssrc,
                                        session_initiation_packet.initiator_token,
                                        &name,
                                    )
                                    .await;
                                }
                                ControlPacket::EndSession => {
                                    Self::handle_end_session(src, &participants);
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
        socket: Arc<Mutex<UdpSocket>>,
        server_name: String,
        ssrc: u32,
        listeners: Arc<Mutex<HashMap<String, Box<dyn Fn(MidiPacket) + Send>>>>,
        participants: Arc<Mutex<HashMap<SocketAddr, Participant>>>,
        session_seq: Arc<Mutex<u16>>,
    ) {
        let mut buf = [0; 65535];
        loop {
            trace!("Listen: Waiting for midi socket lock");
            let socket = socket.lock().await;
            trace!("Listen: Got midi socket lock");
            match socket.recv_from(&mut buf).await {
                Ok((amt, src)) => {
                    trace!("MIDI: Received {} bytes from {}", amt, src);
                    match RtpMidiPacket::parse(&buf[..amt]) {
                        Ok(packet) => {
                            trace!("MIDI: Parsed RTP MIDI packet: {:?}", packet);
                            match packet {
                                RtpMidiPacket::Control(control_packet) => match control_packet {
                                    ControlPacket::SessionInitiation(session_initiation_packet) => {
                                        debug!("MIDI: Received session initiation from {}", src);
                                        Self::send_invitation_response(
                                            &socket,
                                            src,
                                            ssrc,
                                            session_initiation_packet.initiator_token,
                                            &server_name,
                                        )
                                        .await;
                                    }
                                    ControlPacket::ClockSync(clock_sync_packet) => {
                                        debug!("MIDI: Received clock sync from {}", src);
                                        Self::handle_clock_sync(
                                            &socket,
                                            clock_sync_packet,
                                            src,
                                            ssrc,
                                            participants.clone(),
                                        )
                                        .await;
                                    }
                                    _ => {
                                        debug!(
                                            "MIDI: Received control packet: {:?}",
                                            control_packet
                                        );
                                    }
                                },
                                RtpMidiPacket::Midi(midi_packet) => {
                                    debug!("MIDI: Parsed MIDI packet: {:#?}", midi_packet);
                                    // Update sequence number on receive
                                    let mut seq = session_seq.lock().await;
                                    *seq = midi_packet.sequence_number().wrapping_add(1);
                                    if let Some(callback) =
                                        listeners.lock().await.get("midi_packet")
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
        let response_packet = SessionInitiationPacket {
            command: *b"OK",
            protocol_version: 2,
            initiator_token,
            sender_ssrc,
            name: Some(name.to_string()),
        };

        let response_bytes = response_packet.to_bytes();

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
        info!("Control: Ending session with {}", src);
        let participants = Arc::clone(participants);
        tokio::spawn(async move {
            let mut lock = participants.lock().await;
            lock.remove(&src);
        });
    }

    async fn handle_clock_sync(
        socket: &UdpSocket,
        packet: ClockSyncPacket,
        src: std::net::SocketAddr,
        ssrc: u32,
        participants: Arc<Mutex<HashMap<SocketAddr, Participant>>>,
    ) {
        match packet.count {
            0 => {
                // Respond with count = 1
                let timestamp2 = Self::current_timestamp();
                let response_packet = ClockSyncPacket {
                    count: 1,
                    timestamps: vec![packet.timestamps[0], timestamp2, 0],
                    sender_ssrc: ssrc,
                };

                let response_bytes = response_packet.to_bytes();

                if let Err(e) = socket.send_to(&response_bytes, src).await {
                    error!("MIDI: Failed to send clock sync response to {}: {}", src, e);
                } else {
                    debug!("MIDI: Sent clock sync response to {}", src);
                }
            }
            2 => {
                // Finalize clock sync
                info!("MIDI: Clock sync finalized with {}", src);
                let mut lock = participants.lock().await;
                lock.insert(
                    src,
                    Participant {
                        addr: src,
                        last_clock_sync: Instant::now(),
                    },
                );
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
        participants.retain(|_, p| now.duration_since(p.last_clock_sync) < Duration::from_secs(60));
    }

    pub async fn all_participants(&self) -> Vec<SocketAddr> {
        let participants = self.participants.lock().await;
        participants.keys().cloned().collect()
    }

    pub async fn send_midi(&self, commands: &[TimedCommand]) -> std::io::Result<()> {
        let participants = self.all_participants().await;
        trace!("Send: Waiting for midi socket lock");
        let socket = self.midi_socket.lock().await;
        trace!("Send: Got midi socket lock");
        let mut seq = self.sequence_number.lock().await;
        let packet = MidiPacket::new(
            *seq,                             // Sequence number
            Self::current_timestamp() as u32, // Timestamp
            self.ssrc,
            commands,
        );
        *seq = seq.wrapping_add(1); // Increment sequence number, wrapping on overflow
        let mut data = vec![0u8; packet.size()];
        packet.write_to_bytes(&mut data)?;

        info!("Sending MIDI packet to {} participants", participants.len());
        info!("MIDI packet: {:?}", packet);

        info!("Sending MIDI packet to {:?}", participants);
        for addr in participants {
            socket.send_to(&data, addr).await?;
        }
        Ok(())
    }

    fn current_timestamp() -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| std::time::Duration::from_secs(0))
            .as_millis() as u64
    }
}

pub struct Participant {
    pub addr: SocketAddr,
    pub last_clock_sync: Instant,
}

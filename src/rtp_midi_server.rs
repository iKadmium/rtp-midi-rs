use log::{debug, error, info, trace, warn};
use mdns_sd::{ServiceDaemon, ServiceInfo};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::Mutex;
use tokio::task;

use crate::packet::clock_sync_packet::ClockSyncPacket;
use crate::packet::control_packet::ControlPacket;
use crate::packet::midi_packet::midi_packet::MidiPacket;
use crate::packet::packet::RtpMidiPacket;
use crate::packet::session_initiation_packet::SessionInitiationPacket;

pub struct RtpMidiServer {
    name: String,
    ssrc: u32,
    listeners: Arc<Mutex<HashMap<String, Box<dyn Fn(MidiPacket) + Send>>>>,
}

impl RtpMidiServer {
    pub fn new(name: String, ssrc: u32) -> Self {
        Self {
            name,
            ssrc,
            listeners: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn add_listener<F>(&self, event_name: String, callback: F)
    where
        F: Fn(MidiPacket) + Send + 'static,
    {
        let mut listeners = self.listeners.lock().await;
        listeners.insert(event_name, Box::new(callback));
    }

    pub async fn start(&self, control_port: u16) -> std::io::Result<()> {
        let midi_port = control_port + 1;

        // Advertise the service on mDNS
        // Self::advertise_service(&self.name.clone(), midi_port)
        //     .expect("Failed to advertise service");

        let server_name = self.name.clone();
        let listeners_midi = Arc::clone(&self.listeners);

        let control_task = task::spawn(Self::listen_for_control(
            control_port,
            server_name.clone(),
            self.ssrc,
        ));

        let midi_task = task::spawn(Self::listen_for_midi(
            midi_port,
            server_name,
            self.ssrc,
            listeners_midi,
        ));

        println!(
            "RTP MIDI server starting on control port {} and MIDI port {}",
            control_port, midi_port
        );

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

    async fn listen_for_control(control_port: u16, name: String, ssrc: u32) {
        let socket = Arc::new(UdpSocket::bind(("0.0.0.0", control_port)).await.unwrap());
        socket.set_broadcast(true).unwrap();

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
                                    Self::handle_end_session(src);
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
        midi_port: u16,
        server_name: String,
        ssrc: u32,
        listeners: Arc<Mutex<HashMap<String, Box<dyn Fn(MidiPacket) + Send>>>>,
    ) {
        let socket = Arc::new(UdpSocket::bind(("0.0.0.0", midi_port)).await.unwrap());
        socket.set_broadcast(true).unwrap();

        let mut buf = [0; 65535];
        loop {
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

    fn handle_end_session(src: std::net::SocketAddr) {
        info!("Control: Ending session with {}", src);
    }

    async fn handle_clock_sync(
        socket: &UdpSocket,
        packet: ClockSyncPacket,
        src: std::net::SocketAddr,
        ssrc: u32,
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
            }
            _ => {
                error!(
                    "MIDI: Unexpected clock sync count {} from {}",
                    packet.count, src
                );
            }
        }
    }

    fn current_timestamp() -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        (now.as_secs() * 10_000_000) + (now.subsec_nanos() as u64 / 100)
    }
}

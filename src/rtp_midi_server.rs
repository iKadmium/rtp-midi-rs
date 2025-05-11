use log::{debug, error, info, trace, warn};
use std::collections::HashMap;
use std::net::UdpSocket;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::clock_sync_packet::ClockSyncPacket;
use crate::control_packet::ControlPacket;
use crate::midi_command::MidiCommand;
use crate::midi_packet::MidiPacket;
use crate::session_initiation_packet::SessionInitiationPacket;

pub struct RtpMidiServer {
    control_socket: UdpSocket,
    midi_socket: UdpSocket,
    name: String,
    ssrc: u32,
    listeners: Arc<Mutex<HashMap<String, Box<dyn Fn(MidiCommand) + Send>>>>,
}

impl RtpMidiServer {
    pub fn new(
        control_port: u16,
        midi_port: u16,
        name: String,
        ssrc: u32,
    ) -> std::io::Result<Self> {
        let control_socket = UdpSocket::bind(("0.0.0.0", control_port))?;
        let midi_socket = UdpSocket::bind(("0.0.0.0", midi_port))?;
        control_socket.set_read_timeout(Some(Duration::from_secs(1)))?;
        midi_socket.set_read_timeout(Some(Duration::from_secs(1)))?;
        Ok(Self {
            control_socket,
            midi_socket,
            name,
            ssrc,
            listeners: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub fn add_listener<F>(&self, event_name: String, callback: F)
    where
        F: Fn(MidiCommand) + Send + 'static,
    {
        self.listeners
            .lock()
            .unwrap()
            .insert(event_name, Box::new(callback));
    }

    fn emit_event(&self, event_name: &str, data: MidiCommand) {
        if let Some(listener) = self.listeners.lock().unwrap().get(event_name) {
            listener(data);
        }
    }

    pub fn start(&self) -> std::io::Result<()> {
        println!(
            "RTP MIDI server started on control port {} and MIDI port {}",
            self.control_socket.local_addr()?.port(),
            self.midi_socket.local_addr()?.port()
        );

        let control_socket = self.control_socket.try_clone()?;
        let midi_socket = self.midi_socket.try_clone()?;

        let server_name = self.name.clone();
        let listeners_midi = Arc::clone(&self.listeners);

        let control_thread = thread::spawn({
            let server_name = server_name.clone();
            let ssrc = self.ssrc;
            move || Self::listen_for_control(control_socket, server_name, ssrc)
        });
        let midi_thread = thread::spawn({
            let server_name = server_name.clone();
            let ssrc = self.ssrc;
            move || Self::listen_for_midi(midi_socket, server_name, ssrc, listeners_midi)
        });

        control_thread.join().unwrap();
        midi_thread.join().unwrap();

        Ok(())
    }

    fn listen_for_control(socket: UdpSocket, name: String, ssrc: u32) {
        let mut buf = [0; 1024];
        loop {
            match socket.recv_from(&mut buf) {
                Ok((amt, src)) => {
                    trace!("Control: Received {} bytes from {}", amt, src);
                    match ControlPacket::parse_header(&buf[..amt]) {
                        Some(packet) => {
                            trace!("Control: Parsed packet: {:?}", packet);
                            if packet.command.as_deref() == Some("IN") {
                                info!("Control: Received invitation from {}", src);
                                let invitation_packet =
                                    SessionInitiationPacket::parse(&buf[..amt]).unwrap();
                                trace!(
                                    "Control: Parsed invitation packet: {:?}",
                                    invitation_packet
                                );
                                Self::send_invitation_response(
                                    &socket,
                                    src,
                                    ssrc,
                                    invitation_packet.initiator_token,
                                    &name,
                                );
                            } else if packet.command.as_deref() == Some("BY") {
                                trace!("Control: Received end session command from {}", src);
                                Self::handle_end_session(src);
                            } else {
                                warn!("Control: Unhandled command {:?} from {}", packet, src);
                            }
                        }
                        None => {
                            warn!("Control: No valid packet found in data from {}", src);
                        }
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    continue;
                }
                Err(e) => {
                    error!("Control: Error receiving data: {}", e);
                    break;
                }
            }
        }
    }

    fn listen_for_midi(
        socket: UdpSocket,
        name: String,
        ssrc: u32,
        listeners: Arc<Mutex<HashMap<String, Box<dyn Fn(MidiCommand) + Send>>>>,
    ) {
        let mut buf = [0; 1024];
        loop {
            match socket.recv_from(&mut buf) {
                Ok((amt, src)) => {
                    trace!("MIDI: Received {} bytes from {}", amt, src);
                    match ControlPacket::parse_header(&buf[..amt]) {
                        Some(packet) => {
                            trace!("MIDI: Parsed control packet: {:?}", packet);
                            if packet.command.as_deref() == Some("IN") {
                                info!("MIDI: Received invitation from {}", src);
                                let invitation_packet =
                                    SessionInitiationPacket::parse(&buf[..amt]).unwrap();
                                trace!("MIDI: Parsed invitation packet: {:?}", invitation_packet);
                                Self::send_invitation_response(
                                    &socket,
                                    src,
                                    ssrc,
                                    invitation_packet.initiator_token,
                                    &name,
                                );
                            } else if packet.command.as_deref() == Some("CK") {
                                debug!("MIDI: Received clock sync command from {}", src);
                                let clock_sync_packet = ClockSyncPacket::parse(&buf[..amt]);
                                if let Err(e) = clock_sync_packet {
                                    eprintln!(
                                        "MIDI: Failed to parse clock sync packet from {}: {}",
                                        src, e
                                    );
                                    continue;
                                }
                                let clock_sync_packet = clock_sync_packet.unwrap();
                                trace!("MIDI: Parsed clock sync packet: {:?}", clock_sync_packet);
                                Self::handle_clock_sync(&socket, clock_sync_packet, src, ssrc);
                            } else {
                                warn!(
                                    "MIDI: Unhandled control command {:?} from {}",
                                    packet.command, src
                                );
                            }
                        }
                        None => match MidiPacket::parse(&buf[..amt]) {
                            Some(midi_packet) => {
                                debug!("MIDI: Parsed MIDI packet: {:?}", midi_packet);
                                for (time_delta, command) in midi_packet.commands {
                                    std::thread::sleep(Duration::from_micros(
                                        (time_delta * 100) as u64,
                                    ));
                                    listeners
                                        .lock()
                                        .unwrap()
                                        .get("midi_packet")
                                        .map(|callback| callback(command));
                                }
                            }
                            None => {
                                error!("MIDI: No valid packet found in data from {}", src);
                            }
                        },
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    continue;
                }
                Err(e) => {
                    error!("MIDI: Error receiving data: {}", e);
                    break;
                }
            }
        }
    }

    fn send_invitation_response(
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

        if let Err(e) = socket.send_to(&response_bytes, src) {
            error!(
                "{}: Failed to send invitation response to {}: {}",
                socket.local_addr().unwrap().port(),
                src,
                e
            );
        } else {
            trace!(
                "{}: Sent invitation response to {}",
                socket.local_addr().unwrap().port(),
                src
            );
        }
    }

    fn handle_end_session(src: std::net::SocketAddr) {
        info!("Control: Ending session with {}", src);
    }

    fn handle_clock_sync(
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
                    timestamps: [packet.timestamps[0], Some(timestamp2), None],
                    sender_ssrc: ssrc,
                };

                let response_bytes = response_packet.to_bytes();

                if let Err(e) = socket.send_to(&response_bytes, src) {
                    error!("MIDI: Failed to send clock sync response to {}: {}", src, e);
                } else {
                    trace!("MIDI: Sent clock sync response to {}", src);
                }
            }
            2 => {
                // Finalize clock sync
                debug!("MIDI: Clock sync finalized with {}", src);
            }
            _ => {
                eprintln!(
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

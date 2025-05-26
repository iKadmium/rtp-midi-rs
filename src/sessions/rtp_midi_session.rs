use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;

use super::invite_responder::InviteResponder;
use super::mdns::advertise_mdns;
use crate::packets::midi_packets::midi_command::MidiCommand;
use crate::packets::midi_packets::midi_timed_command::TimedCommand;
use crate::sessions::control_port::ControlPort;
use crate::sessions::midi_port::MidiPort;
use crate::{packets::midi_packets::midi_packet::MidiPacket, participant::Participant};

pub(super) type MidiPacketListener = dyn Fn(MidiPacket) + Send + 'static;
pub(super) type ListenerSet = HashMap<RtpMidiEventType, Box<MidiPacketListener>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RtpMidiEventType {
    MidiPacket,
}

#[derive(Clone)]
pub struct RtpMidiSession {
    pub(super) start_time: Instant,
    pub(super) participants: Arc<Mutex<HashMap<u32, Participant>>>, // key by ssrc
    pub(super) sequence_number: Arc<Mutex<u16>>,
    pub(super) listeners: Arc<Mutex<ListenerSet>>,
    pub(super) pending_invitations: Arc<Mutex<HashMap<u32, PendingInvitation>>>, // key by ssrc
    pub(super) control_port: Arc<ControlPort>,
    pub(super) midi_port: Arc<MidiPort>,
}

#[derive(Debug, Clone)]
pub(super) struct PendingInvitation {
    pub addr: SocketAddr,
    pub token: u32,
    pub name: String,
}

impl RtpMidiSession {
    async fn bind(port: u16, name: &str, ssrc: u32) -> std::io::Result<Self> {
        let context = RtpMidiSession {
            start_time: Instant::now(),
            participants: Arc::new(Mutex::new(HashMap::new())),
            sequence_number: Arc::new(Mutex::new(0)),
            pending_invitations: Arc::new(Mutex::new(HashMap::new())),
            control_port: Arc::new(ControlPort::bind(port, name, ssrc).await?),
            midi_port: Arc::new(MidiPort::bind(port + 1, name, ssrc).await?),
            listeners: Arc::new(Mutex::new(HashMap::new())),
        };
        Ok(context)
    }

    pub async fn start(port: u16, name: &str, ssrc: u32, invite_handler: InviteResponder) -> std::io::Result<Arc<Self>> {
        advertise_mdns(name, port).map_err(|e| std::io::Error::other(e.to_string()))?;
        let ctx = Arc::new(Self::bind(port, name, ssrc).await?);
        ctx.start_threads(invite_handler);
        Ok(ctx)
    }

    fn start_threads(&self, invite_handler: InviteResponder) {
        let ctx = Arc::new(self.clone());

        // Control port listener
        let control_port = Arc::clone(&self.control_port);
        let ctx_control = Arc::clone(&ctx);

        tokio::spawn(async move {
            control_port.start(&ctx_control, &invite_handler).await;
        });

        // MIDI port listener
        let ctx_midi = Arc::clone(&ctx);
        let midi_port_listener = Arc::clone(&self.midi_port);
        let listeners_midi = Arc::clone(&self.listeners);
        tokio::spawn(async move {
            midi_port_listener.start_listener(&ctx_midi, listeners_midi).await;
        });

        // MIDI port clock sync
        let ctx_clock = Arc::clone(&ctx);
        let midi_port_clock = Arc::clone(&self.midi_port);
        tokio::spawn(async move {
            midi_port_clock.start_host_clock_sync(&ctx_clock).await;
        });
    }

    pub fn stop(&self) {
        self.control_port.stop();
        self.midi_port.stop();
    }

    pub async fn invite_participant(&self, addr: SocketAddr) -> std::io::Result<()> {
        self.control_port.invite_participant(self, addr).await
    }

    pub async fn participants(&self) -> Vec<Participant> {
        let participants = self.participants.lock().await;
        participants.values().cloned().collect()
    }

    pub async fn remove_participant(&self, participant: &Participant) {
        let _ = self.control_port.send_termination_packet(participant).await;
        let _ = self.midi_port.send_termination_packet(participant).await;
        self.participants.lock().await.remove(&participant.ssrc());
    }

    pub async fn add_listener<F>(&self, event_type: RtpMidiEventType, callback: F)
    where
        F: Fn(MidiPacket) + Send + 'static,
    {
        let mut listeners = self.listeners.lock().await;
        listeners.insert(event_type, Box::new(callback));
    }

    pub async fn send_midi_batch(&self, commands: &[TimedCommand]) -> std::io::Result<()> {
        self.midi_port.send_midi_batch(self, commands).await
    }

    pub async fn send_midi(&self, command: &MidiCommand) -> std::io::Result<()> {
        self.midi_port.send_midi(self, command).await
    }
}

pub fn current_timestamp(start_time: Instant) -> u64 {
    (Instant::now() - start_time).as_micros() as u64 / 100
}

impl Drop for RtpMidiSession {
    fn drop(&mut self) {
        self.stop();
    }
}

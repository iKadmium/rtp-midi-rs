use midi_types::MidiMessage;
use std::time::Duration;
use tokio::sync::broadcast;

use crate::participant::Participant;

/// Event emitted when a participant joins the session.
#[derive(Debug, Clone)]
pub enum ParticipantEvent {
    Joined(Participant),
    Left(Participant),
}

/// Stores broadcast channels for all event types.
pub(super) struct EventListeners {
    midi_messages: broadcast::Sender<Vec<(MidiMessage, Duration)>>,
    sysex: broadcast::Sender<Vec<u8>>,
    participants: broadcast::Sender<ParticipantEvent>,
}

impl EventListeners {
    pub fn new() -> Self {
        let (midi_tx, _) = broadcast::channel(100);
        let (sysex_tx, _) = broadcast::channel(100);
        let (participant_tx, _) = broadcast::channel(100);

        Self {
            midi_messages: midi_tx,
            sysex: sysex_tx,
            participants: participant_tx,
        }
    }

    pub fn subscribe_midi_messages(&self) -> broadcast::Receiver<Vec<(MidiMessage, Duration)>> {
        self.midi_messages.subscribe()
    }

    pub fn subscribe_sysex(&self) -> broadcast::Receiver<Vec<u8>> {
        self.sysex.subscribe()
    }

    pub fn subscribe_participants(&self) -> broadcast::Receiver<ParticipantEvent> {
        self.participants.subscribe()
    }

    /// Notify all listeners of a batch of MIDI messages.
    pub fn notify_midi_messages(&self, events: Vec<(MidiMessage, Duration)>) {
        // Ignore send errors - means no active receivers
        let _ = self.midi_messages.send(events);
    }

    /// Notify all listeners of a SysEx packet.
    pub fn notify_sysex(&self, data: Vec<u8>) {
        let _ = self.sysex.send(data);
    }

    /// Notify all listeners that a participant joined.
    pub fn notify_participant_joined(&self, participant: &Participant) {
        let _ = self.participants.send(ParticipantEvent::Joined(participant.clone()));
    }

    /// Notify all listeners that a participant left.
    pub fn notify_participant_left(&self, participant: &Participant) {
        let _ = self.participants.send(ParticipantEvent::Left(participant.clone()));
    }
}

impl Default for EventListeners {
    fn default() -> Self {
        Self::new()
    }
}

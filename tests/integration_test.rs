mod common;

use common::find_consecutive_ports;
use core::panic;
use midi_types::{Channel, MidiMessage, Note, Value7};
use rtpmidi::sessions::invite_responder::InviteResponder;
use rtpmidi::sessions::rtp_midi_session::RtpMidiSession;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Notify;

#[tokio::test]
async fn test_two_session_inter_communication() {
    let (control_port_1, _midi_port_1) = find_consecutive_ports();
    let (control_port_2, _midi_port_2) = find_consecutive_ports();

    let ssrc1 = 0x11111111;
    let ssrc2 = 0x22222222;
    let session1 = RtpMidiSession::start(control_port_1, "Session1", ssrc1, InviteResponder::Accept)
        .await
        .expect("Failed to start RTP MIDI session");
    let session2 = RtpMidiSession::start(control_port_2, "Session2", ssrc2, InviteResponder::Accept)
        .await
        .expect("Failed to start RTP MIDI session");

    let sessions_connected = Arc::new(Notify::new());

    let mut session1_midi_rx = session1.subscribe_midi_messages().await;
    let mut session1_participant_rx = session1.subscribe_participants().await;
    let mut session2_midi_rx = session2.subscribe_midi_messages().await;

    // Spawn task to wait for participant connection
    let sessions_connected_clone = sessions_connected.clone();
    tokio::spawn(async move {
        if session1_participant_rx.recv().await.is_ok() {
            sessions_connected_clone.notify_one();
        }
    });

    // Invite each other
    let addr1 = SocketAddr::new("127.0.0.1".parse().unwrap(), control_port_1);
    let addr2 = SocketAddr::new("127.0.0.1".parse().unwrap(), control_port_2);
    session1.invite_participant(addr2).await;

    // wait for the sessions to finish connecting
    sessions_connected.notified().await;

    let session1_participants = session1.participants().await;
    let session2_participants = session2.participants().await;
    assert_eq!(session1_participants.len(), 1);
    assert_eq!(session2_participants.len(), 1);
    assert_eq!(session1_participants[0].addr(), addr2);
    assert_eq!(session2_participants[0].addr(), addr1);

    // Send from session1 to session2
    let note_on = MidiMessage::NoteOn(Channel::C1, Note::from(60), Value7::from(100));
    session1.send_midi(&note_on.into()).await.unwrap();

    let events = session2_midi_rx.recv().await.expect("Should receive MIDI events");
    assert!(!events.is_empty(), "Should receive at least one message");
    let (received_message, _delta) = events[0];

    match received_message {
        MidiMessage::NoteOn(channel, note, velocity) => {
            if let MidiMessage::NoteOn(expected_channel, expected_note, expected_velocity) = note_on
            {
                assert_eq!(channel, expected_channel);
                assert_eq!(note, expected_note);
                assert_eq!(velocity, expected_velocity);
            } else {
                panic!("Expected a NoteOn message");
            }
        }
        _ => panic!("Expected a NoteOn message, got {:?}", received_message),
    }

    // Send from session2 to session1
    let note_off = MidiMessage::NoteOff(Channel::C1, Note::from(60), Value7::from(0));
    session2.send_midi(&note_off.into()).await.unwrap();

    let events = session1_midi_rx.recv().await.expect("Should receive MIDI events");
    assert!(!events.is_empty(), "Should receive at least one message");
    let (received_message, _delta) = events[0];

    match received_message {
        MidiMessage::NoteOff(channel, note, velocity) => {
            if let MidiMessage::NoteOff(expected_channel, expected_note, expected_velocity) = note_off
            {
                assert_eq!(channel, expected_channel);
                assert_eq!(note, expected_note);
                assert_eq!(velocity, expected_velocity);
            } else {
                panic!("Expected a NoteOff message");
            }
        }
        _ => panic!("Expected a NoteOff message, got {:?}", received_message),
    }
}

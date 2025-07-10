mod common;

use common::find_consecutive_ports;
use midi_types::{Channel, MidiMessage, Note, Value7};
use rtpmidi::sessions::events::event_handling::MidiMessageEvent;
use rtpmidi::sessions::invite_responder::InviteResponder;
use rtpmidi::sessions::rtp_midi_session::RtpMidiSession;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

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

    // Shared state for assertions
    let received_by_1 = Arc::new(Mutex::new(None));
    let received_by_2 = Arc::new(Mutex::new(None));

    // Listener for session1
    {
        let received_by_1 = received_by_1.clone();
        session1
            .add_listener(MidiMessageEvent, move |message| {
                let received_by_1 = received_by_1.clone();
                tokio::spawn(async move {
                    received_by_1.lock().await.replace(message);
                });
            })
            .await;
    }
    {
        let received_by_2 = received_by_2.clone();
        session2
            .add_listener(MidiMessageEvent, move |message| {
                let received_by_2 = received_by_2.clone();
                tokio::spawn(async move {
                    received_by_2.lock().await.replace(message);
                });
            })
            .await;
    }

    // Give the servers a moment to start
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Invite each other
    let addr1 = SocketAddr::new("127.0.0.1".parse().unwrap(), control_port_1);
    let addr2 = SocketAddr::new("127.0.0.1".parse().unwrap(), control_port_2);
    session1.invite_participant(addr2).await;

    // Wait for sessions to connect
    tokio::time::sleep(Duration::from_secs(1)).await;

    let session1_participants = session1.participants().await;
    let session2_participants = session2.participants().await;
    assert_eq!(session1_participants.len(), 1);
    assert_eq!(session2_participants.len(), 1);
    assert_eq!(session1_participants[0].addr(), addr2);
    assert_eq!(session2_participants[0].addr(), addr1);

    // Send from session1 to session2
    let note_on = MidiMessage::NoteOn(Channel::C1, Note::from(60), Value7::from(100));
    session1.send_midi(&note_on.into()).await.unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    let got = received_by_2.lock().await;
    match got.as_ref() {
        Some(MidiMessage::NoteOn(channel, note, velocity)) => {
            if let MidiMessage::NoteOn(expected_channel, expected_note, expected_velocity) = note_on {
                assert_eq!(channel, &expected_channel);
                assert_eq!(note, &expected_note);
                assert_eq!(velocity, &expected_velocity);
            } else {
                panic!("Expected a NoteOn message");
            }
        }
        _ => panic!("Expected a NoteOn message"),
    }

    // Send from session2 to session1
    let note_off = MidiMessage::NoteOff(Channel::C1, Note::from(60), Value7::from(0));
    session2.send_midi(&note_off.into()).await.unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    let got = received_by_1.lock().await;
    match got.as_ref() {
        Some(MidiMessage::NoteOff(channel, note, velocity)) => {
            if let MidiMessage::NoteOff(expected_channel, expected_note, expected_velocity) = note_off {
                assert_eq!(channel, &expected_channel);
                assert_eq!(note, &expected_note);
                assert_eq!(velocity, &expected_velocity);
            } else {
                panic!("Expected a NoteOff message");
            }
        }
        _ => panic!("Expected a NoteOff message"),
    }
}

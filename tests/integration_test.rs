use rtpmidi::packets::midi_packets::midi_command::MidiCommand;
use rtpmidi::sessions::rtp_midi_session::{RtpMidiEventType, RtpMidiSession};
use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

fn find_consecutive_ports() -> (u16, u16) {
    loop {
        let socket = UdpSocket::bind(("0.0.0.0", 0)).unwrap();
        let port = socket.local_addr().unwrap().port();
        let next_port = port + 1;
        if let Ok(socket2) = UdpSocket::bind(("0.0.0.0", next_port)) {
            drop(socket);
            drop(socket2);
            return (port, next_port);
        }
    }
}

#[tokio::test]
async fn test_two_session_inter_communication() {
    colog::default_builder().filter_level(log::LevelFilter::Debug).init();

    let (control_port_1, _midi_port_1) = find_consecutive_ports();
    let (control_port_2, _midi_port_2) = find_consecutive_ports();

    let ssrc1 = 0x11111111;
    let ssrc2 = 0x22222222;
    let session1 = RtpMidiSession::new("Session1".to_string(), ssrc1);
    let session2 = RtpMidiSession::new("Session2".to_string(), ssrc2);

    // Shared state for assertions
    let received_by_1 = Arc::new(Mutex::new(None));
    let received_by_2 = Arc::new(Mutex::new(None));

    // Listener for session1
    {
        let received_by_1 = received_by_1.clone();
        session1
            .add_listener(RtpMidiEventType::MidiPacket, move |packet| {
                let received_by_1 = received_by_1.clone();
                tokio::spawn(async move {
                    let commands = packet.commands();
                    if let Some(cmd) = commands.first() {
                        *received_by_1.lock().await = Some(cmd.command().clone());
                    }
                });
            })
            .await;
    }
    // Listener for session2
    {
        let received_by_2 = received_by_2.clone();
        session2
            .add_listener(RtpMidiEventType::MidiPacket, move |packet| {
                let received_by_2 = received_by_2.clone();
                tokio::spawn(async move {
                    let commands = packet.commands();
                    if let Some(cmd) = commands.first() {
                        *received_by_2.lock().await = Some(cmd.command().clone());
                    }
                });
            })
            .await;
    }

    // Start both sessions
    session1.start(control_port_1, RtpMidiSession::accept_all_invitations).await.unwrap();
    session2.start(control_port_2, RtpMidiSession::accept_all_invitations).await.unwrap();

    // Give the servers a moment to start
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Invite each other
    let addr1 = SocketAddr::new("127.0.0.1".parse().unwrap(), control_port_1);
    let addr2 = SocketAddr::new("127.0.0.1".parse().unwrap(), control_port_2);
    session1.invite_participant(addr2).await.unwrap();

    // Wait for sessions to connect
    tokio::time::sleep(Duration::from_secs(1)).await;

    let session1_participants = session1.participants().await;
    let session2_participants = session2.participants().await;
    assert_eq!(session1_participants.len(), 1);
    assert_eq!(session2_participants.len(), 1);
    assert_eq!(session1_participants[0].addr(), addr2);
    assert_eq!(session2_participants[0].addr(), addr1);

    // Send from session1 to session2
    let note_on = MidiCommand::NoteOn {
        channel: 1,
        key: 60,
        velocity: 100,
    };
    session1.send_midi(&note_on).await.unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    let got = received_by_2.lock().await.clone();
    assert_eq!(got, Some(note_on.clone()));

    // Send from session2 to session1
    let note_off = MidiCommand::NoteOff {
        channel: 1,
        key: 60,
        velocity: 0,
    };
    session2.send_midi(&note_off).await.unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;
    let got = received_by_1.lock().await.clone();
    assert_eq!(got, Some(note_off.clone()));
}

#[tokio::test]
async fn test_stop_cleanup() {
    let (control_port, midi_port) = find_consecutive_ports();

    let ssrc = 0x11111111;
    let session = Arc::new(RtpMidiSession::new("Cleanup".to_string(), ssrc));

    session.start(control_port, RtpMidiSession::accept_all_invitations).await.unwrap();
    session.stop().await;

    drop(session);

    tokio::time::sleep(Duration::from_millis(500)).await;

    // Check if the socket is closed
    let _control_port_socket = UdpSocket::bind((std::net::Ipv4Addr::UNSPECIFIED, control_port)).expect("Failed to bind control port");
    let _midi_port_socket = UdpSocket::bind((std::net::Ipv4Addr::UNSPECIFIED, midi_port)).expect("Failed to bind MIDI port");
}

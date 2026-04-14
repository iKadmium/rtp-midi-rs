#[cfg(feature = "examples")]
#[tokio::main]
async fn main() {
    use rtpmidi::sessions::{
        events::ParticipantEvent, invite_responder::InviteResponder,
        rtp_midi_session::RtpMidiSession,
    };
    use tracing::{event, Level};
    use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    let session = RtpMidiSession::start(5004, "Multi Stream", 12345, InviteResponder::Accept)
        .await
        .expect("Failed to start RTP-MIDI session");

    // Subscribe to multiple event streams
    let mut midi_rx = session.subscribe_midi_messages().await;
    let mut sysex_rx = session.subscribe_sysex().await;
    let mut participant_rx = session.subscribe_participants().await;

    event!(
        Level::INFO,
        "Session started. Listening to all event types. Press Ctrl+C to stop."
    );

    // Handle multiple streams with tokio::select!
    loop {
        tokio::select! {
            Ok(events) = midi_rx.recv() => {
                event!(Level::INFO, "MIDI batch: {} messages", events.len());
                for (message, delta) in events {
                    event!(Level::DEBUG, "  {:?} (delta: {:?})", message, delta);
                }
            }

            Ok(data) = sysex_rx.recv() => {
                event!(Level::INFO, "SysEx packet: {} bytes", data.len());
            }

            Ok(event) = participant_rx.recv() => {
                match event {
                    ParticipantEvent::Joined(p) => {
                        event!(Level::INFO, "✓ Participant joined: {}", p.name().to_string_lossy());
                    }
                    ParticipantEvent::Left(p) => {
                        event!(Level::INFO, "✗ Participant left: {}", p.name().to_string_lossy());
                    }
                }
            }

            _ = tokio::signal::ctrl_c() => {
                println!("Ctrl+C received, stopping session...");
                event!(Level::INFO, "Stopping RTP-MIDI session gracefully");
                session.stop_gracefully().await;
                break;
            }
        }
    }
}

#[cfg(not(feature = "examples"))]
fn main() {
    println!("This example requires the 'examples' feature to be enabled.");
}

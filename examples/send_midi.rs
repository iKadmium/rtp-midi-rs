#[cfg(feature = "examples")]
#[tokio::main]
async fn main() {
    use midi_types::{MidiMessage, Note};
    use rtpmidi::sessions::{invite_responder::InviteResponder, rtp_midi_session::RtpMidiSession};
    use tracing::{event, Level};
    use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    let session = RtpMidiSession::start(5004, "My Session", 54321, InviteResponder::Accept)
        .await
        .expect("Failed to start RTP-MIDI session");

    let session_clone = session.clone();
    let mut midi_rx = session.subscribe_midi_messages().await;

    event!(Level::INFO, "Session started, listening for MIDI messages...");

    loop {
        tokio::select! {
            Ok(events) = midi_rx.recv() => {
                // Process each MIDI message in the batch
                for (message, _delta) in events {
                    // Filter for NoteOn commands
                    if let MidiMessage::NoteOn(channel, key, velocity) = message {
                        let response = MidiMessage::NoteOn(
                            channel,
                            Note::from(u8::from(key) - 12), // Down 1 octave
                            velocity,
                        );

                        let session_clone = session_clone.clone();
                        tokio::spawn(async move {
                            match session_clone.send_midi(&response.into()).await {
                                Ok(_) => event!(
                                    Level::INFO,
                                    "MIDI packet sent successfully, {:?}",
                                    response
                                ),
                                Err(e) => event!(Level::ERROR, "Error sending MIDI packet: {:?}", e),
                            };
                        });
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

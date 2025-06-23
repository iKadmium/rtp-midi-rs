#[cfg(feature = "examples")]
#[tokio::main]
async fn main() {
    use rtpmidi::packets::midi_packets::midi_command::MidiCommand;
    use rtpmidi::sessions::{
        invite_responder::InviteResponder,
        rtp_midi_session::{RtpMidiEventType, RtpMidiSession},
    };
    use tracing::{Level, event};
    use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

    tracing_subscriber::registry().with(fmt::layer()).with(EnvFilter::from_default_env()).init();

    let session = RtpMidiSession::start(5004, "My Session", 54321, InviteResponder::Accept)
        .await
        .expect("Failed to start RTP-MIDI session");

    let session_clone = session.clone();

    // Add a listener for incoming MIDI packets
    session
        .add_listener(RtpMidiEventType::MidiPacket, move |command| {
            // Filter for NoteOn commands

            if let MidiCommand::NoteOn { channel, key, velocity } = command {
                let response = MidiCommand::NoteOn {
                    channel: *channel,
                    key: *key - 12, // Down 1 octave
                    velocity: *velocity,
                };

                let session_clone = session_clone.clone();
                tokio::spawn(async move {
                    match session_clone.send_midi(&response).await {
                        Ok(_) => event!(Level::INFO, "MIDI packet sent successfully, {:?}", response),
                        Err(e) => event!(Level::INFO, "Error sending MIDI packet: {:?}", e),
                    };
                });
            }
        })
        .await;

    // Wait for the server task to complete (keeps process alive)
    tokio::signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
    session.stop_gracefully().await;
}

#[cfg(not(feature = "examples"))]
fn main() {
    println!("This example requires the 'examples' feature to be enabled.");
}

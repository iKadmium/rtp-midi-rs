#[cfg(feature = "examples")]
#[tokio::main]
async fn main() {
    use rtpmidi::sessions::{
        events::MidiScheduler, invite_responder::InviteResponder, rtp_midi_session::RtpMidiSession,
    };
    use tracing::{event, Level};
    use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    let session = RtpMidiSession::start(5004, "Scheduled Session", 12345, InviteResponder::Accept)
        .await
        .expect("Failed to start RTP-MIDI session");

    // Create a scheduler that plays messages at the correct time
    let scheduler = MidiScheduler::new(|message| {
        event!(Level::INFO, "Playing MIDI message: {:?}", message);
        // In a real application, this would trigger a synthesizer or MIDI output
    });

    let mut midi_rx = session.subscribe_midi_messages().await;

    event!(
        Level::INFO,
        "Session started. Waiting for MIDI messages. Press Ctrl+C to stop."
    );

    loop {
        tokio::select! {
            Ok(events) = midi_rx.recv() => {
                event!(
                    Level::DEBUG,
                    "Received batch of {} MIDI messages",
                    events.len()
                );
                // The scheduler will play each message at the correct time based on delta times
                scheduler.schedule(events);
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

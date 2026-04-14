#[cfg(feature = "examples")]
#[tokio::main]
async fn main() {
    use rtpmidi::sessions::{invite_responder::InviteResponder, rtp_midi_session::RtpMidiSession};
    use tracing::{event, Level};
    use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    let session = RtpMidiSession::start(5004, "My Session", 12345, InviteResponder::Accept)
        .await
        .expect("Failed to start RTP-MIDI session");

    let mut midi_rx = session.subscribe_midi_messages().await;
    let mut sysex_rx = session.subscribe_sysex().await;

    event!(Level::INFO, "Session started, waiting for MIDI messages...");

    loop {
        tokio::select! {
            Ok(events) = midi_rx.recv() => {
                for (message, delta) in events {
                    event!(
                        Level::INFO,
                        "Received MIDI message: {:?}, delta: {:?}",
                        message,
                        delta
                    );
                }
            }
            Ok(data) = sysex_rx.recv() => {
                event!(Level::INFO, "Received SysEx packet: {} bytes", data.len());
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

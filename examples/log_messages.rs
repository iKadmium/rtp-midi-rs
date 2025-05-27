#[cfg(feature = "examples")]
#[tokio::main]
async fn main() {
    use rtpmidi::sessions::{
        invite_responder::InviteResponder,
        rtp_midi_session::{RtpMidiEventType, RtpMidiSession},
    };
    use tracing::{Level, event};
    use tracing_subscriber::util::SubscriberInitExt;

    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_target(false)
        .finish()
        .init();

    let server = RtpMidiSession::start(5004, "My Session", 12345, InviteResponder::Accept)
        .await
        .expect("Failed to start RTP-MIDI session");

    server
        .add_listener(RtpMidiEventType::MidiPacket, move |data| {
            for command in data.commands() {
                event!(Level::INFO, "Received command: {:?}", command);
            }
        })
        .await;

    tokio::signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
}

#[cfg(not(feature = "examples"))]
fn main() {
    println!("This example requires the 'examples' feature to be enabled.");
}

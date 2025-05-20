#[cfg(feature = "examples")]
use tokio; // Add tokio runtime for async main

#[cfg(feature = "examples")]
#[tokio::main]
async fn main() {
    use log::info;
    use rtpmidi::rtp_midi_session::{RtpMidiEventType, RtpMidiSession};

    colog::default_builder().filter_level(log::LevelFilter::Trace).init();

    let server = RtpMidiSession::new("My Session".to_string(), 54321, 5004).await.unwrap();

    server
        .add_listener(RtpMidiEventType::MidiPacket, move |data| {
            for command in data.commands() {
                info!("Received command: {:?}", command);
            }
        })
        .await;

    // Wait for the server task to complete (keeps process alive)
    let _ = server.start(RtpMidiSession::accept_all_invitations).await;
}

#[cfg(not(feature = "examples"))]
fn main() {
    println!("This example requires the 'examples' feature to be enabled.");
}

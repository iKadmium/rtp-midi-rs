#[cfg(feature = "examples")]
#[tokio::main]
async fn main() {
    use log::info;
    use rtpmidi::sessions::rtp_midi_session::{RtpMidiEventType, RtpMidiSession};

    colog::default_builder().filter_level(log::LevelFilter::Info).init();

    let server = RtpMidiSession::new("My Session".to_string(), 5004);

    server
        .add_listener(RtpMidiEventType::MidiPacket, move |data| {
            for command in data.commands() {
                info!("Received command: {:?}", command);
            }
        })
        .await;

    server.start(5004, RtpMidiSession::accept_all_invitations).await.unwrap();

    tokio::signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
}

#[cfg(not(feature = "examples"))]
fn main() {
    println!("This example requires the 'examples' feature to be enabled.");
}

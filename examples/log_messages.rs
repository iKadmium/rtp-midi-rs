#[cfg(feature = "examples")]
#[tokio::main]
async fn main() {
    use log::info;
    use rtpmidi::sessions::{
        invite_response::InviteResponse,
        rtp_midi_session::{RtpMidiEventType, RtpMidiSession},
    };

    colog::default_builder().filter_level(log::LevelFilter::Info).init();

    let server = RtpMidiSession::start(5004, "My Session", 12345, InviteResponse::Accept)
        .await
        .expect("Failed to start RTP-MIDI session");

    server
        .add_listener(RtpMidiEventType::MidiPacket, move |data| {
            for command in data.commands() {
                info!("Received command: {:?}", command);
            }
        })
        .await;

    tokio::signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
}

#[cfg(not(feature = "examples"))]
fn main() {
    println!("This example requires the 'examples' feature to be enabled.");
}

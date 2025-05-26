use log::info;
use rtpmidi::packets::midi_packets::{midi_command::MidiCommand, midi_timed_command::TimedCommand};
use rtpmidi::sessions::invite_response::InviteResponse;
use rtpmidi::sessions::rtp_midi_session::RtpMidiSession;

#[cfg(feature = "examples")]
#[tokio::main]
async fn main() {
    use rtpmidi::sessions::rtp_midi_session::RtpMidiEventType;

    colog::default_builder().filter_level(log::LevelFilter::Info).init();

    let session = RtpMidiSession::start(5004, "My Session", 54321, InviteResponse::Accept)
        .await
        .expect("Failed to start RTP-MIDI session");

    let session_clone = session.clone();

    // Add a listener for incoming MIDI packets
    session
        .add_listener(RtpMidiEventType::MidiPacket, move |data| {
            // Filter for NoteOn commands
            let commands: Vec<TimedCommand> = data
                .commands()
                .iter()
                .filter_map(|c| match c.command() {
                    // Return a NoteOn command down 1 octave
                    MidiCommand::NoteOn { channel, key, velocity } => Some(TimedCommand::new(
                        None,
                        MidiCommand::NoteOn {
                            channel: *channel,
                            key: key.saturating_sub(12),
                            velocity: *velocity,
                        },
                    )),
                    _ => None,
                })
                .collect();

            if !commands.is_empty() {
                let session_clone = session_clone.clone();
                tokio::spawn(async move {
                    match session_clone.send_midi_batch(&commands).await {
                        Ok(_) => info!("MIDI packet sent successfully, {:?}", commands),
                        Err(e) => info!("Error sending MIDI packet: {:?}", e),
                    };
                });
            }
        })
        .await;

    // Wait for the server task to complete (keeps process alive)
    tokio::signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
}

#[cfg(not(feature = "examples"))]
fn main() {
    println!("This example requires the 'examples' feature to be enabled.");
}

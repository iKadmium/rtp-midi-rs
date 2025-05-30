#[cfg(feature = "examples")]
#[tokio::main]
async fn main() {
    use std::sync::Arc;

    use log::info;
    use rtpmidi::packets::midi_packets::{midi_command::MidiCommand, midi_timed_command::TimedCommand};
    use rtpmidi::sessions::rtp_midi_session::{RtpMidiEventType, RtpMidiSession};

    colog::default_builder().filter_level(log::LevelFilter::Info).init();

    let session = Arc::new(RtpMidiSession::new("My Session".to_string(), 54321));

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

    session
        .start(5004, RtpMidiSession::accept_all_invitations)
        .await
        .expect("Error while running the server");

    // Wait for the server task to complete (keeps process alive)
    tokio::signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
}

#[cfg(not(feature = "examples"))]
fn main() {
    println!("This example requires the 'examples' feature to be enabled.");
}

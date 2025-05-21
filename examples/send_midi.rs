#[cfg(feature = "examples")]
#[tokio::main]
async fn main() {
    use std::sync::Arc;

    use log::info;
    use rtpmidi::packets::midi_packets::{midi_command::MidiCommand, midi_timed_command::TimedCommand};
    use rtpmidi::rtp_midi_session::{RtpMidiEventType, RtpMidiSession};

    colog::default_builder().filter_level(log::LevelFilter::Trace).init();

    let server = Arc::new(RtpMidiSession::new("My Session".to_string(), 54321, 5004).await.unwrap());

    // Add a listener for incoming MIDI packets
    let server_clone = server.clone();
    server
        .add_listener(RtpMidiEventType::MidiPacket, move |data| {
            let server_clone = server_clone.clone();
            tokio::spawn(async move {
                // Filter for NoteOn commands
                let commands: Vec<TimedCommand> = data
                    .commands()
                    .iter()
                    .filter_map(|c| match c.command() {
                        MidiCommand::NoteOn { channel, key, velocity } => Some(TimedCommand::new(
                            None,
                            MidiCommand::NoteOn {
                                channel: *channel,
                                key: key.saturating_sub(12), // Transpose down by 1 octave
                                velocity: *velocity,
                            },
                        )),
                        _ => None,
                    })
                    .collect();

                if !commands.is_empty() {
                    match server_clone.send_midi_batch(&commands).await {
                        Ok(_) => info!("MIDI packet sent successfully, {:?}", commands),
                        Err(e) => info!("Error sending MIDI packet: {:?}", e),
                    };
                }
            });
        })
        .await;

    // Start the server in a background task
    let server_task = {
        let server = server.clone();
        tokio::spawn(async move {
            server
                .start(RtpMidiSession::accept_all_invitations)
                .await
                .expect("Error while running the server");
        })
    };

    // Wait for the server task to complete (keeps process alive)
    let _ = server_task.await;
}

#[cfg(not(feature = "examples"))]
fn main() {
    println!("This example requires the 'examples' feature to be enabled.");
}

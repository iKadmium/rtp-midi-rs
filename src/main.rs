use std::sync::Arc;

use log::info;
use rtpmidi::rtp_midi_session::{RtpMidiEventType, RtpMidiSession};
use tokio; // Add tokio runtime for async main

use rtpmidi::packet::midi_packets::{
    midi_command::MidiCommand, midi_packet::MidiPacket, midi_timed_command::TimedCommand,
};

#[tokio::main]
async fn main() {
    colog::default_builder()
        .filter_level(log::LevelFilter::Info)
        .init();

    let server = Arc::new(
        RtpMidiSession::new("RTPMidiServer".to_string(), 12345, 5004)
            .await
            .unwrap(),
    );

    let server_clone = server.clone();
    server
        .add_listener(RtpMidiEventType::MidiPacket, move |data| {
            let server_clone = server_clone.clone();
            tokio::spawn(async move {
                handle_midi_packet(&data);

                let commands: Vec<TimedCommand> = data
                    .commands()
                    .iter()
                    .filter_map(|c| match c.command() {
                        MidiCommand::NoteOn {
                            channel,
                            key,
                            velocity,
                        } => Some(TimedCommand::new(
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
                    match server_clone.send_midi_batch(&commands).await {
                        Ok(_) => info!("MIDI packet sent successfully, {:?}", commands),
                        Err(e) => info!("Error sending MIDI packet: {:?}", e),
                    };
                }
            });
        })
        .await;

    server
        .start()
        .await
        .expect("Error while running the server");
}

fn handle_midi_packet(data: &MidiPacket) {
    for command in data.commands() {
        info!("Received command: {:?}", command);
    }
}

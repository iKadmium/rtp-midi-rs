mod packet;
mod rtp_midi_session;

use std::sync::Arc;

use log::info;
use packet::midi_packet::{
    midi_command::MidiCommand, midi_packet::MidiPacket, midi_timed_command::TimedCommand,
};
use rtp_midi_session::RtpMidiSession;
use tokio; // Add tokio runtime for async main

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
        .add_listener("midi_packet".to_string(), move |data| {
            let server = server_clone.clone();
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
                    match server.send_midi(&commands).await {
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

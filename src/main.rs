mod packet;
mod rtp_midi_server;

use log::info;
use packet::midi_packet::midi_packet::MidiPacket;
use rtp_midi_server::RtpMidiServer;
use tokio; // Add tokio runtime for async main

#[tokio::main]
async fn main() {
    colog::default_builder()
        .filter_level(log::LevelFilter::Info)
        .init();

    let server = RtpMidiServer::new("RTPMidiServer".to_string(), 12345);

    server
        .add_listener("midi_packet".to_string(), handle_midi_packet)
        .await;

    server
        .start(5004)
        .await
        .expect("Error while running the server");
}

fn handle_midi_packet(data: MidiPacket) {
    for command in data.commands {
        info!("Received command: {:?}", command);
    }
}

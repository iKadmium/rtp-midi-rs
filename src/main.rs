mod packet;
mod rtp_midi_server;

use log::info;
use rtp_midi_server::RtpMidiServer;

fn main() {
    colog::default_builder()
        .filter_level(log::LevelFilter::Info)
        .init();

    let server = RtpMidiServer::new(5004, 5005, "RTPMidiServer".to_string(), 12345)
        .expect("Failed to start RTP MIDI server");

    server.add_listener("midi_packet".to_string(), |data| {
        info!("MIDI Packet Event: {:?}", data);
    });

    server.start().expect("Error while running the server");
}

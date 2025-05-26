use rtpmidi::sessions::rtp_midi_session::RtpMidiSession;

#[cfg(feature = "examples")]
#[tokio::main]
async fn main() {
    use std::net::SocketAddr;

    use rtpmidi::sessions::invite_response::InviteResponse;

    colog::default_builder().filter_level(log::LevelFilter::Info).init();

    let session = RtpMidiSession::start(
        5004,
        "My Session",
        54321,
        InviteResponse::new(|packet, _addr| packet.name() == Some("Bob's jam session")),
    )
    .await
    .expect("Failed to start RTP MIDI session");

    let addr = SocketAddr::new("172.31.112.1".parse().unwrap(), 5006);
    session.invite_participant(addr).await.unwrap();

    tokio::signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
}

#[cfg(not(feature = "examples"))]
fn main() {
    println!("This example requires the 'examples' feature to be enabled.");
}

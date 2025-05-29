use rtpmidi::sessions::invite_responder::InviteResponder;
use rtpmidi::sessions::rtp_midi_session::RtpMidiSession;
use std::net::SocketAddr;
use tracing::Level;

#[cfg(feature = "examples")]
#[tokio::main]
async fn main() {
    use tracing_subscriber::util::SubscriberInitExt;

    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_target(false)
        .finish()
        .init();

    let session = RtpMidiSession::start(
        5004,
        "My Session",
        54321,
        InviteResponder::new(|packet, _addr| packet.name() == Some("Bob's jam session")),
    )
    .await
    .expect("Failed to start RTP MIDI session");

    let addr = SocketAddr::new("172.31.112.1".parse().unwrap(), 5006);
    session.invite_participant(addr).await;

    tokio::signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
}

#[cfg(not(feature = "examples"))]
fn main() {
    println!("This example requires the 'examples' feature to be enabled.");
}

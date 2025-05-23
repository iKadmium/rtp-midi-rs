use rtpmidi::sessions::rtp_midi_session::RtpMidiSession;
use std::sync::Arc;

#[cfg(feature = "examples")]
#[tokio::main]
async fn main() {
    colog::default_builder().filter_level(log::LevelFilter::Info).init();

    let server = Arc::new(RtpMidiSession::new("My Session".to_string(), 54321));
    server
        .start(5004, RtpMidiSession::accept_all_invitations)
        .await
        .expect("Error while running the server");

    let addr = std::net::SocketAddr::new("172.31.112.1".parse().unwrap(), 5006);
    server.invite_participant(addr).await.unwrap();

    tokio::signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
}

#[cfg(not(feature = "examples"))]
fn main() {
    println!("This example requires the 'examples' feature to be enabled.");
}

#[cfg(feature = "examples")]
use tokio; // Add tokio runtime for async main

#[cfg(feature = "examples")]
#[tokio::main]
async fn main() {
    use std::sync::Arc;

    use rtpmidi::rtp_midi_session::RtpMidiSession;

    colog::default_builder().filter_level(log::LevelFilter::Trace).init();

    let server = Arc::new(RtpMidiSession::new("My Session".to_string(), 54321, 5004).await.unwrap());

    // Start the server in a background task
    let server_task = {
        let server = server.clone();
        tokio::spawn(async move {
            server.start().await.expect("Error while running the server");
        })
    };

    let invite_server = server.clone();
    let _ = tokio::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        let addr = std::net::SocketAddr::new("172.31.112.1".parse().unwrap(), 5006);
        let _ = invite_server.invite_participant(addr).await.unwrap();
    })
    .await;

    // Wait for the server task to complete (keeps process alive)
    let _ = server_task.await;
}

#[cfg(not(feature = "examples"))]
fn main() {
    println!("This example requires the 'examples' feature to be enabled.");
}

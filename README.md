# rtpmidi
[![Crates.io Version](https://img.shields.io/crates/v/rtpmidi)](https://crates.io/crates/rtpmidi)
![Crates.io License](https://img.shields.io/crates/l/rtpmidi)
![Codecov](https://img.shields.io/codecov/c/github/iKadmium/rtp-midi-rs)

This provides functions for working with RTP-MIDI in Rust.

## Usage

```rs
let session = RtpMidiSession::new("My Session".to_string(), 54321, 5004).await.unwrap();

// Wait for midi commands
session
    .add_listener(RtpMidiEventType::MidiPacket, move |data| {
        for command in data.commands() {
            info!("Received command: {:?}", command);
        }
    })
    .await;

// start listening for packets - this can be accept_all_invitations, reject_all_invitations, or a function
// that takes the form my_function(packet: &SessionInitiationPacket, socket: &SocketAddr) -> bool
let _ = server.start(RtpMidiSession::accept_all_invitations).await;

// invite another participant to the session
let addr = std::net::SocketAddr::new("192.168.0.1".parse().unwrap(), 5006);
let _ = invite_server.invite_participant(addr).await.unwrap();

// send MIDI commands
let command = MidiCommand::NoteOn {
    channel: *channel,
    key: key.saturating_sub(12), // Transpose down by 1 octave
    velocity: *velocity,
};

session.send_midi(command).await.unwrap();
```

See the Examples directory for more examples.

## Installation

```cargo add rtpmidi```

## Status

Supported:  
* Responding to invitations (currently auto-accepts all invitations)
* Inviting others
* Advertising via MDNS / Bonjour (optional - enable the 'mdns' feature for this)
* SysEx

Not supported:  
* Recovery journal
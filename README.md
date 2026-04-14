# rtpmidi
[![Crates.io Version](https://img.shields.io/crates/v/rtpmidi)](https://crates.io/crates/rtpmidi)
![Crates.io License](https://img.shields.io/crates/l/rtpmidi)
![Codecov](https://img.shields.io/codecov/c/github/iKadmium/rtp-midi-rs)

This provides functions for working with RTP-MIDI in Rust.

## Usage

```rs
use rtpmidi::sessions::{rtp_midi_session::RtpMidiSession, invite_responder::InviteResponder};
use midi_types::{Channel, MidiMessage, Note, Value7};
use std::net::SocketAddr;

let port = 5004_u16;
let ssrc = 123456_u32;
let session = RtpMidiSession::start(port, "My Session", ssrc, InviteResponder::Accept)
    .await
    .unwrap();

// Subscribe to MIDI messages
let mut midi_rx = session.subscribe_midi_messages().await;

tokio::spawn(async move {
    while let Ok(events) = midi_rx.recv().await {
        for (message, delta) in events {
            println!("Received: {:?}, delta: {:?}", message, delta);
        }
    }
});

// Or use the scheduler helper for automatic timing
use rtpmidi::sessions::events::MidiScheduler;

let scheduler = MidiScheduler::new(|message| {
    // This will be called at the correct time based on delta times
    println!("Play: {:?}", message);
});

let mut midi_rx = session.subscribe_midi_messages().await;
tokio::spawn(async move {
    while let Ok(events) = midi_rx.recv().await {
        scheduler.schedule(events);
    }
});

// Invite another participant to the session
let addr = SocketAddr::new("192.168.0.1".parse().unwrap(), 5006);
session.invite_participant(addr).await;

// Send MIDI messages
let message = MidiMessage::NoteOn(Channel::C1, Note::from(64), Value7::from(127));
session.send_midi(&message.into()).await.unwrap();
```

See the Examples directory for more examples.

## Installation

```cargo add rtpmidi```

## Status

Supported:  
* Responding to invitations
* Inviting others
* Advertising via MDNS / Bonjour (optional - enable the 'mdns' feature for this)
* SysEx

Not supported:  
* Recovery journal
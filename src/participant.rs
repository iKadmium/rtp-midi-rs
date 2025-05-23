use std::{net::SocketAddr, time::Instant};

#[derive(Debug, Clone, PartialEq)]
pub struct Participant {
    ctrl_addr: SocketAddr,
    initiator_token: Option<u32>,
    last_clock_sync: Instant,
    name: String,
    invited_by_us: bool, // Indicates if the participant was invited by us
}

impl Participant {
    pub fn new(ctrl_addr: SocketAddr, invited_by_us: bool, initiator_token: Option<u32>, name: String) -> Self {
        Participant {
            ctrl_addr,
            initiator_token,
            name,
            last_clock_sync: Instant::now(),
            invited_by_us,
        }
    }

    pub fn midi_port_addr(&self) -> SocketAddr {
        SocketAddr::new(self.ctrl_addr.ip(), self.ctrl_addr.port() + 1)
    }

    pub fn last_clock_sync(&self) -> Instant {
        self.last_clock_sync
    }

    pub fn received_clock_sync(&mut self) {
        self.last_clock_sync = Instant::now();
    }

    pub fn is_invited_by_us(&self) -> bool {
        self.invited_by_us
    }

    pub fn initiator_token(&self) -> Option<u32> {
        self.initiator_token
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn addr(&self) -> SocketAddr {
        self.ctrl_addr
    }
}

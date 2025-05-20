use std::{net::SocketAddr, time::Instant};

#[derive(Debug, Clone)]
pub struct Participant {
    addr: SocketAddr,
    last_clock_sync: Instant,
    invited_by_us: bool, // Indicates if the participant was invited by us
}

impl Participant {
    pub fn new(addr: SocketAddr, invited_by_us: bool) -> Self {
        Participant {
            addr,
            last_clock_sync: Instant::now(),
            invited_by_us,
        }
    }

    pub fn control_port_addr(&self) -> SocketAddr {
        SocketAddr::new(self.addr.ip(), self.addr.port())
    }

    pub fn midi_port_addr(&self) -> SocketAddr {
        SocketAddr::new(self.addr.ip(), self.addr.port() + 1)
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
}

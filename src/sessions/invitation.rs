struct Invitation {
    token: u32,
    name: String,
    ssrc: u32,
}

impl Invitation {
    pub fn new(token: u32, name: String, ssrc: u32) -> Self {
        Invitation { token, name, ssrc }
    }

    pub fn token(&self) -> u32 {
        self.token
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn ssrc(&self) -> u32 {
        self.ssrc
    }
}

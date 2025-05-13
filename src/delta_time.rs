#[derive(Debug, Clone, PartialEq)]
pub struct DeltaTime {
    delta_time: u32,
}

impl DeltaTime {
    pub fn new(delta_time: u32) -> Self {
        DeltaTime { delta_time }
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), String> {
        let mut delta_time = 0u32;
        let mut i = 0;

        loop {
            if i >= bytes.len() {
                return Err("Not enough data for delta time".to_string());
            }
            let byte = bytes[i];
            i += 1;
            delta_time = (delta_time << 7) | (byte & 0b0111_1111) as u32;
            if byte & 0b1000_0000 == 0 {
                break;
            }
        }

        Ok((DeltaTime::new(delta_time), i))
    }
}

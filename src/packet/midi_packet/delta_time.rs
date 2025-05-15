#[derive(Clone, PartialEq)]
pub struct DeltaTime {
    chunks: Vec<DeltaTimeChunk>,
}

type DeltaTimeChunk = u8;

pub trait DeltaTimeChunkExt {
    fn continuation(&self) -> bool;
    fn time(&self) -> u8;
}

const CONTINUATION_MASK: u8 = 0b1000_0000;
const TIME_MASK: u8 = 0b0111_1111;
const U28_MAX: u32 = 0b0000_1111_1111_1111_1111_1111_1111_1111;
const U21_MAX: u32 = 0b0000_0000_0001_1111_1111_1111_1111_1111;
const U14_MAX: u32 = 0b0000_0000_0000_0000_0011_1111_1111_1111;
const U7_MAX: u32 = 0b0000_0000_0000_0000_0000_0000_0111_1111;

impl DeltaTimeChunkExt for u8 {
    fn continuation(&self) -> bool {
        self & CONTINUATION_MASK != 0
    }

    fn time(&self) -> u8 {
        self & TIME_MASK
    }
}

impl DeltaTime {
    pub fn zero() -> Self {
        Self { chunks: vec![0] }
    }

    pub fn new(delta_time: u32) -> Self {
        if delta_time > U28_MAX {
            panic!("Delta time exceeds maximum value of {}", U28_MAX);
        }
        if delta_time > U21_MAX {
            let chunks = vec![
                ((delta_time >> 21) & U7_MAX) as u8,
                ((delta_time >> 14) & U7_MAX) as u8 | CONTINUATION_MASK,
                ((delta_time >> 7) & U7_MAX) as u8 | CONTINUATION_MASK,
                (delta_time & U7_MAX) as u8 | CONTINUATION_MASK,
            ];
            return DeltaTime { chunks };
        } else if delta_time > U14_MAX {
            let chunks = vec![
                ((delta_time >> 14) & U7_MAX) as u8,
                ((delta_time >> 7) & U7_MAX) as u8 | CONTINUATION_MASK,
                (delta_time & U7_MAX) as u8 | CONTINUATION_MASK,
            ];
            return DeltaTime { chunks };
        } else if delta_time > U7_MAX {
            let chunks = vec![
                ((delta_time >> 7) & U7_MAX) as u8,
                (delta_time & U7_MAX) as u8 | CONTINUATION_MASK,
            ];
            return DeltaTime { chunks };
        } else {
            let chunks = vec![(delta_time & U7_MAX) as u8];
            return DeltaTime { chunks };
        }
    }

    pub fn time(&self) -> u32 {
        let mut time = 0;
        for (i, chunk) in self.chunks.iter().enumerate() {
            time |= ((*chunk & TIME_MASK) as u32) << (7 * (self.chunks.len() - i - 1));
        }
        return time;
    }

    pub fn size(&self) -> usize {
        return self.chunks.len();
    }

    pub fn from_be_bytes(bytes: &[u8]) -> Result<Self, std::io::Error> {
        let mut chunks = Vec::new();
        for i in 0..4 {
            let chunk = bytes[i];
            chunks.push(chunk);
            if !chunk.continuation() {
                break;
            }
        }

        Ok(DeltaTime { chunks })
    }

    pub fn write_to_bytes(&self, bytes: &mut [u8]) -> Result<usize, std::io::Error> {
        bytes[0..self.size()].copy_from_slice(&self.chunks);

        Ok(self.size())
    }
}

impl std::fmt::Debug for DeltaTime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DeltaTime {{ {:?} }}", self.time())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delta_time_one_byte() {
        let delta_time = DeltaTime::new(0b0000_1111);
        assert_eq!(delta_time.time(), 0b0000_1111);
        assert_eq!(delta_time.size(), 1);

        let mut bytes = [0; 1];
        delta_time.write_to_bytes(&mut bytes).unwrap();
        assert_eq!(bytes, [0b0000_1111]);
    }

    #[test]
    fn test_delta_time_two_bytes() {
        let delta_time = DeltaTime::new(0b1111_1111);
        assert_eq!(delta_time.time(), 0b1111_1111);
        assert_eq!(delta_time.size(), 2);

        let mut bytes = [0; 2];
        delta_time.write_to_bytes(&mut bytes).unwrap();
        assert_eq!(bytes, [0b0000_0001, 0b1111_1111]);
    }

    #[test]
    fn test_delta_time_three_bytes() {
        let delta_time = DeltaTime::new(0b1111_1111_1111_1111);
        assert_eq!(delta_time.time(), 0b1111_1111_1111_1111);
        assert_eq!(delta_time.size(), 3);

        let mut bytes = [0; 3];
        delta_time.write_to_bytes(&mut bytes).unwrap();
        assert_eq!(bytes, [0b0000_0011, 0b1111_1111, 0b1111_1111]);
    }

    #[test]
    fn test_delta_time_four_bytes() {
        let delta_time = DeltaTime::new(0b0011_1111_1111_1111_1111_1111);
        assert_eq!(delta_time.time(), 0b0011_1111_1111_1111_1111_1111);
        assert_eq!(delta_time.size(), 4);

        let mut bytes = [0; 4];
        delta_time.write_to_bytes(&mut bytes).unwrap();
        assert_eq!(bytes, [0b0000_0001, 0b1111_1111, 0b1111_1111, 0b1111_1111]);
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum DeltaTime {
    OneByte([u8; 1]),
    TwoByte([u8; 2]),
    ThreeByte([u8; 3]),
    FourByte([u8; 4]),
}

pub trait DeltaTimeChunkExt {
    fn continuation(&self) -> bool;
    fn time(&self) -> u8;
}

const CONTINUATION_MASK: u8 = 0b1000_0000;
const TIME_MASK: u8 = 0b0111_1111;
const U28_MAX: u32 = 0b0000_1111_1111_1111_1111_1111_1111_1111;
const U21_MAX: u32 = 0b0000_0000_0001_1111_1111_1111_1111_1111;
const U14_MAX: u32 = 0b0000_0000_0000_0000_0011_1111_1111_1111;

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
        DeltaTime::OneByte([0])
    }

    pub fn new(delta_time: u32) -> Self {
        assert!(
            delta_time <= U28_MAX,
            "Delta time exceeds maximum value of {}",
            U28_MAX
        );
        if delta_time <= TIME_MASK as u32 {
            // 1 byte
            DeltaTime::OneByte([delta_time as u8])
        } else if delta_time <= U14_MAX {
            // 2 bytes, big-endian
            let b1 = ((delta_time >> 7) as u8) | CONTINUATION_MASK;
            let b2 = (delta_time & TIME_MASK as u32) as u8;
            DeltaTime::TwoByte([b1, b2])
        } else if delta_time <= U21_MAX {
            // 3 bytes, big-endian
            let b1 = ((delta_time >> 14) as u8) | CONTINUATION_MASK;
            let b2 = (((delta_time >> 7) & TIME_MASK as u32) as u8) | CONTINUATION_MASK;
            let b3 = (delta_time & TIME_MASK as u32) as u8;
            DeltaTime::ThreeByte([b1, b2, b3])
        } else {
            // 4 bytes, big-endian
            let b1 = ((delta_time >> 21) as u8) | CONTINUATION_MASK;
            let b2 = (((delta_time >> 14) & TIME_MASK as u32) as u8) | CONTINUATION_MASK;
            let b3 = (((delta_time >> 7) & TIME_MASK as u32) as u8) | CONTINUATION_MASK;
            let b4 = (delta_time & TIME_MASK as u32) as u8;
            DeltaTime::FourByte([b1, b2, b3, b4])
        }
    }

    pub fn time(&self) -> u32 {
        match self {
            DeltaTime::OneByte(arr) => DeltaTime::decode(arr),
            DeltaTime::TwoByte(arr) => DeltaTime::decode(arr),
            DeltaTime::ThreeByte(arr) => DeltaTime::decode(arr),
            DeltaTime::FourByte(arr) => DeltaTime::decode(arr),
        }
    }

    fn decode(arr: &[u8]) -> u32 {
        let mut time = 0u32;
        for (i, chunk) in arr.iter().enumerate() {
            time |= ((*chunk & TIME_MASK) as u32) << (7 * (arr.len() - i - 1));
        }
        time
    }

    pub fn size(&self) -> usize {
        match self {
            DeltaTime::OneByte(_) => 1,
            DeltaTime::TwoByte(_) => 2,
            DeltaTime::ThreeByte(_) => 3,
            DeltaTime::FourByte(_) => 4,
        }
    }

    pub fn delta_time_byte_length(bytes: &[u8]) -> Result<usize, std::io::Error> {
        match bytes
            .iter()
            .enumerate()
            .find(|&(_index, byte)| !byte.continuation())
        {
            Some((index, _byte)) => {
                if index > 3 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "DeltaTime length exceeds 4 bytes",
                    ));
                }
                Ok(index + 1)
            }
            None => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "DeltaTime continuation bit never cleared",
            )),
        }
    }

    pub fn from_be_bytes(bytes: &[u8]) -> Result<Self, std::io::Error> {
        if bytes.is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "No bytes for DeltaTime",
            ));
        }
        let length = Self::delta_time_byte_length(bytes)?;

        let delta_time_bytes = &bytes[..length];
        match length {
            1 => Ok(DeltaTime::OneByte(delta_time_bytes.try_into().unwrap())),
            2 => Ok(DeltaTime::TwoByte(delta_time_bytes.try_into().unwrap())),
            3 => Ok(DeltaTime::ThreeByte(delta_time_bytes.try_into().unwrap())),
            4 => Ok(DeltaTime::FourByte(delta_time_bytes.try_into().unwrap())),
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid DeltaTime length",
            )),
        }
    }

    pub fn write_to_bytes(&self, bytes: &mut [u8]) -> Result<usize, std::io::Error> {
        let arr: &[u8] = match self {
            DeltaTime::OneByte(a) => a,
            DeltaTime::TwoByte(a) => a,
            DeltaTime::ThreeByte(a) => a,
            DeltaTime::FourByte(a) => a,
        };
        if bytes.len() < arr.len() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::WriteZero,
                "Not enough space to write DeltaTime",
            ));
        }
        bytes[..arr.len()].copy_from_slice(arr);
        Ok(arr.len())
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
        assert_eq!(bytes, [0b1000_0001, 0b0111_1111]);
    }

    #[test]
    fn test_delta_time_three_bytes() {
        let delta_time = DeltaTime::new(0b1111_1111_1111_1111);
        assert_eq!(delta_time.time(), 0b1111_1111_1111_1111);
        assert_eq!(delta_time.size(), 3);

        let mut bytes = [0; 3];
        delta_time.write_to_bytes(&mut bytes).unwrap();
        assert_eq!(bytes, [0b1000_0011, 0b1111_1111, 0b0111_1111]);
    }

    #[test]
    fn test_delta_time_four_bytes() {
        let delta_time = DeltaTime::new(0b0011_1111_1111_1111_1111_1111);
        assert_eq!(delta_time.time(), 0b0011_1111_1111_1111_1111_1111);
        assert_eq!(delta_time.size(), 4);

        let mut bytes = [0; 4];
        delta_time.write_to_bytes(&mut bytes).unwrap();
        assert_eq!(bytes, [0b1000_0001, 0b1111_1111, 0b1111_1111, 0b0111_1111]);
    }

    #[test]
    fn test_delta_time_from_bytes() {
        let bytes: [u8; 4] = [0b1000_0001, 0b1111_1111, 0b1111_1111, 0b0111_1111];
        let delta_time = DeltaTime::from_be_bytes(&bytes).unwrap();
        assert_eq!(delta_time.size(), 4);
        match delta_time {
            DeltaTime::FourByte(arr) => {
                assert_eq!(arr, [0b1000_0001, 0b1111_1111, 0b1111_1111, 0b0111_1111]);
            }
            _ => panic!("Expected Four bytes"),
        }
        assert_eq!(delta_time.time(), 0b0011_1111_1111_1111_1111_1111)
    }
}

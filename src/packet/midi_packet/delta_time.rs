#[derive(Clone, PartialEq)]
pub struct DeltaTime {
    chunks: Vec<DeltaTimeChunk>,
}

type DeltaTimeChunk = u8;

pub trait DeltaTimeChunkExt {
    fn continuation(&self) -> bool;
    fn time(&self) -> u8;
}

impl DeltaTimeChunkExt for u8 {
    fn continuation(&self) -> bool {
        self & 0b1000_0000 != 0
    }

    fn time(&self) -> u8 {
        self & 0b0111_1111
    }
}

impl DeltaTime {
    pub fn zero() -> Self {
        Self { chunks: vec![0] }
    }

    pub fn new(delta_time: u32) -> Self {
        let mut chunks = Vec::new();
        let mut remaining_time = delta_time;

        while remaining_time > 0 {
            let time = (remaining_time & 0x7F) as u8;
            // Compose the chunk byte: set the high bit if more chunks follow
            let continuation = if remaining_time > 0x7F {
                0b1000_0000
            } else {
                0
            };
            chunks.push(continuation | time);
            remaining_time >>= 7;
        }

        DeltaTime { chunks }
    }

    pub fn time(&self) -> u32 {
        let mut time = 0;
        for (i, chunk) in self.chunks.iter().enumerate() {
            time |= (*chunk as u32 & 0x7F) << (7 * i);
        }
        time
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

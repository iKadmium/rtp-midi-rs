use bitstream_io::FromBitStream;

#[derive(Debug, Clone, PartialEq)]
pub struct DeltaTime {
    pub time: u32,
}

struct DeltaTimeChunk {
    continuation: bool,
    time: u8,
}

impl FromBitStream for DeltaTimeChunk {
    type Error = std::io::Error;

    fn from_reader<R: bitstream_io::BitRead + ?Sized>(reader: &mut R) -> Result<Self, Self::Error>
    where
        Self: Sized,
    {
        let continuation = reader.read_bit()?;
        let time = reader.read::<7, _>()?;
        Ok(DeltaTimeChunk { continuation, time })
    }
}

impl DeltaTime {
    pub fn new(delta_time: u32) -> Self {
        DeltaTime { time: delta_time }
    }
}

impl DeltaTime {
    pub fn from_reader<R: bitstream_io::BitRead + ?Sized>(
        reader: &mut R,
    ) -> Result<(Self, usize), std::io::Error>
    where
        Self: Sized,
    {
        let mut delta_time = 0u32;
        let mut bytes_read = 0;

        loop {
            let chunk = reader.parse::<DeltaTimeChunk>()?;
            bytes_read += 1;
            delta_time = (delta_time << 7) | (chunk.time as u32);
            if !chunk.continuation {
                break;
            }
        }

        Ok((DeltaTime::new(delta_time), bytes_read))
    }
}

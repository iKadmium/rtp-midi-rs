use bitstream_io::FromBitStream;

#[derive(Debug, Clone, PartialEq)]
pub struct DeltaTime {
    chunks: Vec<DeltaTimeChunk>,
}

#[derive(Debug, Clone, PartialEq)]
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
    pub const ZERO: u8 = 0b1000_0000;

    pub fn new(delta_time: u32) -> Self {
        let mut chunks = Vec::new();
        let mut remaining_time = delta_time;

        while remaining_time > 0 {
            let time = (remaining_time & 0x7F) as u8;
            let continuation = remaining_time > 0x7F;
            chunks.push(DeltaTimeChunk { continuation, time });
            remaining_time >>= 7;
        }

        DeltaTime { chunks }
    }

    pub fn size(&self) -> usize {
        return self.chunks.len();
    }

    pub fn to_writer<W: bitstream_io::BitWrite + ?Sized>(
        &self,
        writer: &mut W,
    ) -> std::io::Result<()> {
        for chunk in &self.chunks {
            writer.write_bit(chunk.continuation)?;
            writer.write::<7, _>(chunk.time)?;
        }
        Ok(())
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

use bitstream_io::FromBitStream;

#[derive(Debug)]
#[allow(dead_code)]
pub struct ProgramChangeChapter {
    pub s: bool,
    pub program: u8,
    pub b: bool,
    pub bank_msb: u8,
    pub x: bool,
    pub bank_lsb: u8,
}

impl FromBitStream for ProgramChangeChapter {
    type Error = std::io::Error;

    fn from_reader<R: bitstream_io::BitRead + ?Sized>(reader: &mut R) -> Result<Self, Self::Error> {
        let s = reader.read_bit()?;
        let program = reader.read::<7, u8>()?;
        let b = reader.read_bit()?;
        let bank_msb = reader.read::<7, u8>()?;
        let x = reader.read_bit()?;
        let bank_lsb = reader.read::<7, u8>()?;

        Ok(Self {
            s,
            program,
            b,
            bank_msb,
            x,
            bank_lsb,
        })
    }
}

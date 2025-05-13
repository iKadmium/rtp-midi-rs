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

impl ProgramChangeChapter {
    pub fn parse(data: &[u8]) -> Result<(Self, usize), String> {
        if data.len() < 3 {
            return Err("Data length is less than 3".to_string());
        }

        let s = (data[0] & 0b1000_0000) != 0;
        let program = data[0] & 0b0111_1111;
        let b = (data[1] & 0b1000_0000) != 0;
        let bank_msb = data[1] & 0b0111_1111;
        let x = (data[2] & 0b1000_0000) != 0;
        let bank_lsb = data[2] & 0b0111_1111;

        Ok((
            Self {
                s,
                program,
                b,
                bank_msb,
                x,
                bank_lsb,
            },
            3,
        ))
    }
}

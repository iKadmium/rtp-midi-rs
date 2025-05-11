use super::recovery_journal::Chapter;

#[derive(Debug)]
pub struct ProgramChangeChapter {
    pub s: bool,
    pub program: u8,
    pub b: bool,
    pub bank_msb: u8,
    pub x: bool,
    pub bank_lsb: u8,
}

impl Chapter for ProgramChangeChapter {
    fn parse(data: &[u8]) -> Option<(Self, usize)> {
        if data.len() < 3 {
            return None;
        }

        let s = (data[0] & 0b1000_0000) != 0;
        let program = data[0] & 0b0111_1111;
        let b = (data[1] & 0b1000_0000) != 0;
        let bank_msb = data[1] & 0b0111_1111;
        let x = (data[2] & 0b1000_0000) != 0;
        let bank_lsb = data[2] & 0b0111_1111;

        Some((
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

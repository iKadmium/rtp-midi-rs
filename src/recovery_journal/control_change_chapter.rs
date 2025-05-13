#[derive(Debug, PartialEq)]
pub struct ControlChangeChapter {
    pub entries: Vec<ControlChangeEntry>,
}

#[derive(Debug, PartialEq)]
pub struct ControlChangeEntry {
    pub number: u8,
    pub value: u8,
    pub value_type: ControlChangeChapterValueType,
}

#[derive(Debug, PartialEq)]
pub enum ControlChangeChapterValueType {
    Value,
    Toggle,
    Count,
}

impl ControlChangeChapter {
    pub fn parse(data: &[u8]) -> Result<(Self, usize), String> {
        let mut entries = Vec::new();
        let mut index = 0;

        let length = data[0] as usize;

        while index + 2 < data.len() {
            let number = data[index + 1] & 0b0111_1111;
            let a_flag = (data[index + 1] & 0b1000_0000) != 0;
            let (value, value_type) = if a_flag {
                let toggle = data[index + 2] & 0b0100_0000 == 0;
                let value = data[index + 2] & 0b0011_1111;
                let value_type = if toggle {
                    ControlChangeChapterValueType::Toggle
                } else {
                    ControlChangeChapterValueType::Count
                };
                (value, value_type)
            } else {
                (
                    data[index + 2] & 0b0111_1111,
                    ControlChangeChapterValueType::Value,
                )
            };

            entries.push(ControlChangeEntry {
                number,
                value,
                value_type,
            });
            index += 3;
        }

        Ok((ControlChangeChapter { entries }, length))
    }
}

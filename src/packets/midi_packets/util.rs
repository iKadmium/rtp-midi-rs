pub(crate) trait StatusBit {
    fn status_bit(&self) -> bool;
    fn non_status_byte(&self) -> u8;
}

impl StatusBit for u8 {
    fn status_bit(&self) -> bool {
        self & 0x80 != 0
    }

    fn non_status_byte(&self) -> u8 {
        self & 0x7F
    }
}

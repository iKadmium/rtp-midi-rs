use super::util::StatusBit;

#[derive(Debug)]
#[allow(dead_code)]
pub struct DeltaTimeZeroAlloc<'a> {
    data: &'a [u8],
}

#[allow(dead_code)]
impl<'a> DeltaTimeZeroAlloc<'a> {
    pub fn from_be_bytes(bytes: &'a [u8]) -> (Self, usize) {
        let len = bytes.iter().position(|b| !b.status_bit()).unwrap();
        let delta_time = &bytes[..len];
        (Self { data: delta_time }, len)
    }

    pub fn delta_time(&self) -> u32 {
        let mut delta_time = 0u32;
        for &byte in self.data {
            delta_time = (delta_time << 7) | (byte.non_status_byte()) as u32;
        }
        delta_time
    }
}

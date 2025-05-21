use byteorder::ReadBytesExt;
use std::io::Read;

// Extension trait to read an optional, null-terminated string
pub trait ReadOptionalStringExt: Read {
    fn read_optional_string(&mut self) -> std::io::Result<Option<String>>;
}

impl<R: Read> ReadOptionalStringExt for R {
    fn read_optional_string(&mut self) -> std::io::Result<Option<String>> {
        let mut name_bytes = Vec::<u8>::new();
        loop {
            match self.read_u8() {
                Ok(0) => {
                    // Null terminator found
                    return Ok(Some(
                        String::from_utf8(name_bytes).map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "Name contains invalid UTF-8"))?,
                    ));
                }
                Ok(byte) => {
                    name_bytes.push(byte);
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    // EOF reached.
                    if name_bytes.is_empty() {
                        // Optional name was not present.
                        return Ok(None);
                    } else {
                        // Name started but was not null-terminated before EOF. This is a malformed packet.
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "Name field present but not null-terminated before EOF",
                        ));
                    }
                }
                Err(e) => {
                    // Any other I/O error
                    return Err(e);
                }
            }
        }
    }
}

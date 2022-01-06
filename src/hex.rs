use std::fmt;

pub(crate) struct HexBytes<'a> {
    bytes: &'a [u8],
}

impl<'a> HexBytes<'a> {
    pub(crate) const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }
}

impl<'a> fmt::Debug for HexBytes<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.bytes.fmt(f)
    }
}

impl<'a> fmt::Display for HexBytes<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.bytes
            .iter()
            .copied()
            .map(byte_to_hex)
            .map(|(h1, h2)| write!(f, "{}{}", h1, h2))
            .collect()
    }
}

fn byte_to_hex(byte: u8) -> (char, char) {
    (nibble_to_hex(byte >> 4), nibble_to_hex(byte & 0x0F))
}

fn nibble_to_hex(nibble: u8) -> char {
    (nibble
        + match nibble {
            0..=9 => 0x30,
            _ => 0x41,
        }) as char
}

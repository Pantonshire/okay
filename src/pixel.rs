#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Pixel {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Pixel {
    pub const ZERO: Self = Self::new(0, 0, 0, 0);
    pub const BLACK: Self = Self::new(0, 0, 0, u8::MAX);

    #[inline]
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    #[inline]
    pub const fn rgba(self) -> [u8; 4] {
        [self.r, self.g, self.b, self.a]
    }

    #[inline]
    pub const fn argb(self) -> [u8; 4] {
        [self.a, self.r, self.g, self.b]
    }

    #[inline]
    pub const fn rgb(self) -> [u8; 3] {
        [self.r, self.g, self.b]
    }
}

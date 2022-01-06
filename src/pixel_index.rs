use crate::pixel::Pixel;

pub(crate) struct PixelIndex {
    inner: [Pixel; Self::SIZE],
}

impl PixelIndex {
    const SIZE: usize = 64;

    /// Creates a new zero-initialised index
    pub fn new() -> Self {
        Self {
            inner: [Pixel::ZERO; Self::SIZE],
        }
    }

    /// Gets the pixel from the index referred to by the given QOI_OP_INDEX chunk
    pub fn masked_get(&self, chunk: u8) -> Pixel {
        // SAFETY:
        // Masking chunk by `Self::SIZE - 1` (0b00111111) limits it to 6 bits, so it can never
        // exceed 63. Since the length of the array is 64, this means that it can never be
        // out-of-bounds
        unsafe { *self.inner.get_unchecked((chunk & (Self::SIZE as u8 - 1)) as usize) }
    }

    /// Inserts the given pixel into the index at the position corresponding to its hash value
    pub fn insert(&mut self, pixel: Pixel) {
        self.inner[Self::pixel_hash(pixel)] = pixel;
    }

    #[inline(always)]
    fn pixel_hash(pixel: Pixel) -> usize {
        (pixel.r as usize * 3 + pixel.g as usize * 5 + pixel.b as usize * 7 + pixel.a as usize * 11)
            % Self::SIZE
    }
}

use std::convert;
use std::error;
use std::fmt;
use std::io;
use std::slice;

use crate::byte_stream::SliceByteStream;
use crate::byte_stream::{
    ByteStream, IntoStreamResult, IterByteStream, ReadByteStream, StreamError,
};
use crate::header::{self, Header};
use crate::pixel::Pixel;
use crate::pixel_index::PixelIndex;

// TODO: "the byte stream's end is marked with 7 0x00 bytes followed by a single 0x01 byte"

pub struct Decoder<S> {
    stream: S,
}

impl<'a> Decoder<SliceByteStream<'a>> {
    pub fn new_from_slice(slice: &'a [u8]) -> Self {
        Self::new(slice.into())
    }
}

impl<I, T> Decoder<IterByteStream<I>>
where
    I: Iterator<Item = T>,
    T: IntoStreamResult,
{
    pub fn new_from_iter<J>(iter: J) -> Self
    where
        J: IntoIterator<IntoIter = I>,
    {
        Self::new(iter.into_iter().into())
    }
}

impl<R> Decoder<ReadByteStream<R>>
where
    R: io::Read,
{
    pub fn new_from_reader(reader: R) -> Self {
        Self::new(reader.into())
    }
}

impl<S> Decoder<S>
where
    S: ByteStream,
{
    pub fn new(stream: S) -> Self {
        Self { stream }
    }

    pub fn decode_header(
        mut self,
    ) -> Result<(Header, PixelDecoder<S>), HeaderDecodeError<S::IoError>> {
        Header::validate_magic(self.stream.read_n()?)?;

        let width = u32::from_be_bytes(self.stream.read_n()?);
        let height = u32::from_be_bytes(self.stream.read_n()?);
        let channels = self.stream.read_one()?.try_into()?;
        let col_space = self.stream.read_one()?.try_into()?;

        let num_pixels = width as u64 * height as u64;

        Ok((
            Header::new(width, height, channels, col_space),
            PixelDecoder::new(self.stream, num_pixels),
        ))
    }
}

pub struct PixelDecoder<S> {
    stream: S,
    previous: Pixel,
    index: PixelIndex,
    remaining: u64,
    run: u8,
}

impl<S> PixelDecoder<S>
where
    S: ByteStream,
{
    fn new(stream: S, num_pixels: u64) -> Self {
        Self {
            stream,
            previous: Pixel::BLACK,
            index: PixelIndex::new(),
            remaining: num_pixels,
            run: 0,
        }
    }

    /// Decodes pixels into the given buffer until the buffer becomes full or the end of the image is
    /// reached, whichever comes first. The returned `usize` is the number of pixels written to the
    /// buffer. The returned `bool` is true if all of the pixels have been decoded after this operation,
    /// and false if there are still more pixels that need to be decoded after this operation.
    pub fn decode_pixels_into(
        &mut self,
        buf: &mut [Pixel],
    ) -> Result<(usize, bool), StreamError<S::IoError>> {
        self.decode_into_pixel_buf(buf, convert::identity)
    }

    /// Allocates a new vec large enough for all of the remaining pixels, decodes all of the remaining
    /// pixels into the vec, and returns the buffer. Returns a `DecodeAllError::TooLarge` if allocating
    /// a vec large enough is not possible.
    pub fn decode_pixels_vec(mut self) -> Result<Vec<Pixel>, DecodeAllError<S::IoError>> {
        let num_pixels = self.remaining.try_into().map_err(|_| DecodeAllError::TooLarge)?;

        let mut buf = Vec::new();
        buf.try_reserve_exact(num_pixels)
            .map_err(|_| DecodeAllError::TooLarge)?;

        let ptr = buf.as_mut_ptr();
        let dst = unsafe { slice::from_raw_parts_mut(ptr, num_pixels) };

        let (n, exhausted) = self.decode_into_pixel_buf(dst, convert::identity)?;

        unsafe {
            buf.set_len(n);
        }

        debug_assert!(exhausted);

        Ok(buf)
    }

    pub fn decode_bytes_into<F, const N: usize>(
        &mut self,
        buf: &mut [u8],
        transform: F,
    ) -> Result<(usize, bool), StreamError<S::IoError>>
    where
        F: Fn(Pixel) -> [u8; N],
    {
        assert!(N != 0);

        let num_pixels = buf.len() / N;

        let ptr = buf.as_mut_ptr() as *mut [u8; N];
        let dst = unsafe { slice::from_raw_parts_mut(ptr, num_pixels) };

        self.decode_into_pixel_buf(dst, transform)
            .map(|(n, exhausted)| (n * N, exhausted))
    }

    pub fn decode_bytes_vec<F, const N: usize>(
        mut self,
        transform: F,
    ) -> Result<Vec<u8>, DecodeAllError<S::IoError>>
    where
        F: Fn(Pixel) -> [u8; N],
    {
        assert!(N != 0);

        let num_pixels = usize::try_from(self.remaining)
            .map_err(|_| DecodeAllError::TooLarge)?;

        let num_bytes = num_pixels.checked_mul(N)
            .ok_or(DecodeAllError::TooLarge)?;

        let mut buf = Vec::new();
        buf.try_reserve_exact(num_bytes)
            .map_err(|_| DecodeAllError::TooLarge)?;

        let ptr = buf.as_mut_ptr() as *mut [u8; N];
        let dst = unsafe { slice::from_raw_parts_mut(ptr, num_pixels) };

        let (n, exhausted) = self.decode_into_pixel_buf(dst, transform)?;

        unsafe {
            buf.set_len(n * N);
        }

        debug_assert!(exhausted);

        Ok(buf)
    }

    /// Returns the number of pixels remaining to be decoded. This is an upper bound on how many more
    /// pixels the decoder can output; fewer pixels will be output if the byte stream ends prematurely or
    /// contains invalid data.
    pub fn remaining_pixels(&self) -> u64 {
        self.remaining
    }

    fn decode_into_pixel_buf<T, F>(
        &mut self,
        buf: &mut [T],
        transform: F,
    ) -> Result<(usize, bool), StreamError<S::IoError>>
    where
        F: Fn(Pixel) -> T,
    {
        let (num_pixels, exhausted) = if !usize::try_from(self.remaining).is_ok() || buf.len() < self.remaining as usize {
            (buf.len(), false)
        } else {
            (self.remaining as usize, true)
        };

        for i in 0..num_pixels {
            if self.run > 0 {
                self.run -= 1;
                buf[i] = transform(self.previous);
                continue;
            }

            let b0 = self.stream.read_one()?;

            match b0 {
                // QOI_OP_RGB
                0xFE => {
                    let [r, g, b] = self.stream.read_n()?;
                    self.previous.r = r;
                    self.previous.g = g;
                    self.previous.b = b;
                    self.index.insert(self.previous);
                }

                // QOI_OP_RGBA
                0xFF => {
                    let [r, g, b, a] = self.stream.read_n()?;
                    self.previous = Pixel::new(r, g, b, a);
                    self.index.insert(self.previous);
                }

                _ => match b0 >> 6 {
                    // QOI_OP_INDEX
                    0x0 => {
                        self.previous = self.index.masked_get(b0);
                    }

                    // QOI_OP_DIFF
                    0x1 => {
                        self.previous.r = self
                            .previous
                            .r
                            .wrapping_sub(2)
                            .wrapping_add((b0 >> 4) & 0x3);
                        self.previous.g = self
                            .previous
                            .g
                            .wrapping_sub(2)
                            .wrapping_add((b0 >> 2) & 0x3);
                        self.previous.b = self.previous.b.wrapping_sub(2).wrapping_add(b0 & 0x3);
                        self.index.insert(self.previous);
                    }

                    // QOI_OP_LUMA
                    0x2 => {
                        let b1 = self.stream.read_one()?;
                        let dg = (b0 & 0x3F).wrapping_sub(32);
                        self.previous.r = self
                            .previous
                            .r
                            .wrapping_add(dg)
                            .wrapping_sub(8)
                            .wrapping_add((b1 >> 4) & 0x0F);
                        self.previous.g = self.previous.g.wrapping_add(dg);
                        self.previous.b = self
                            .previous
                            .b
                            .wrapping_add(dg)
                            .wrapping_sub(8)
                            .wrapping_add(b1 & 0x0F);
                        self.index.insert(self.previous);
                    }

                    // QOI_OP_RUN
                    _ => {
                        self.run = b0 & 0x3F;
                    }
                },
            }

            buf[i] = transform(self.previous);
        }

        self.remaining -= num_pixels as u64;
        Ok((num_pixels, exhausted))
    }
}

#[derive(Debug)]
pub enum HeaderDecodeError<E> {
    UnexpectedEof,
    Io(E),
    Magic(header::MagicError),
    Channels(header::ChannelsError),
    ColSpace(header::ColSpaceError),
}

impl<E> fmt::Display for HeaderDecodeError<E>
where
    E: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedEof => f.write_str("unexpected eof"),
            Self::Io(err) => err.fmt(f),
            Self::Magic(err) => err.fmt(f),
            Self::Channels(err) => err.fmt(f),
            Self::ColSpace(err) => err.fmt(f),
        }
    }
}

impl<E> error::Error for HeaderDecodeError<E> where E: error::Error {}

impl<E> From<StreamError<E>> for HeaderDecodeError<E> {
    fn from(err: StreamError<E>) -> Self {
        match err {
            StreamError::UnexpectedEof => Self::UnexpectedEof,
            StreamError::Io(err) => Self::Io(err),
        }
    }
}

impl<E> From<header::MagicError> for HeaderDecodeError<E> {
    fn from(err: header::MagicError) -> Self {
        Self::Magic(err)
    }
}

impl<E> From<header::ChannelsError> for HeaderDecodeError<E> {
    fn from(err: header::ChannelsError) -> Self {
        Self::Channels(err)
    }
}

impl<E> From<header::ColSpaceError> for HeaderDecodeError<E> {
    fn from(err: header::ColSpaceError) -> Self {
        Self::ColSpace(err)
    }
}

#[derive(Debug)]
pub enum DecodeAllError<E> {
    UnexpectedEof,
    TooLarge,
    Io(E),
}

impl<E> fmt::Display for DecodeAllError<E>
where
    E: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedEof => f.write_str("unexpected eof"),
            Self::TooLarge => f.write_str("image too large"),
            Self::Io(err) => err.fmt(f),
        }
    }
}

impl<E> error::Error for DecodeAllError<E> where E: error::Error {}

impl<E> From<StreamError<E>> for DecodeAllError<E> {
    fn from(err: StreamError<E>) -> Self {
        match err {
            StreamError::UnexpectedEof => Self::UnexpectedEof,
            StreamError::Io(err) => Self::Io(err),
        }
    }
}

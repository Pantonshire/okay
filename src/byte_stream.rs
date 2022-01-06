use std::convert::Infallible;
use std::error;
use std::fmt;
use std::io::{self, Read};
use std::mem::MaybeUninit;

/// A trait representing a fallible sequence of bytes, which may be infinite or finite.
pub trait ByteStream {
    type IoError;

    /// Returns the next `N` bytes in the sequence. If there are fewer than `N` bytes remaining in
    /// the sequence, a `StreamError::UnexpectedEof` should be returned. Implmentors of the trait
    /// can also define an IO error type, which they may return if some IO error occurs while
    /// generating the bytes.
    fn read_n<const N: usize>(&mut self) -> Result<[u8; N], StreamError<Self::IoError>>;

    /// A specialised version of `read_n` that returns just the next byte in the sequence.
    #[inline]
    fn read_one(&mut self) -> Result<u8, StreamError<Self::IoError>> {
        self.read_n().map(|[b]| b)
    }
}

pub struct SliceByteStream<'a> {
    slice: &'a [u8],
}

impl<'a> SliceByteStream<'a> {
    pub fn new(slice: &'a [u8]) -> Self {
        Self { slice }
    }

    pub fn inner(&self) -> &'a [u8] {
        self.slice
    }
}

impl<'a> From<&'a [u8]> for SliceByteStream<'a> {
    fn from(slice: &'a [u8]) -> Self {
        Self::new(slice)
    }
}

impl<'a> ByteStream for SliceByteStream<'a> {
    // Reading from a slice can never encounter an IO error, so use `Infallible` which can never
    // be constructed. Note that `StreamError<Infallible>` is a ZST because it only has one variant
    // that is actually possible to construct
    type IoError = Infallible;

    fn read_n<const N: usize>(&mut self) -> Result<[u8; N], StreamError<Self::IoError>> {
        if self.slice.len() < N {
            Err(StreamError::UnexpectedEof)
        } else {
            let ptr = self.slice.as_ptr() as *const [u8; N];
            // SAFETY:
            // We have already checked that the length of the slice is at least N, so `ptr` is a
            // valid pointer to `[u8; N]`. Therefore, dereferencing the pointer is safe
            let bytes = unsafe { *ptr };
            self.slice = &self.slice[N..];
            Ok(bytes)
        }
    }

    fn read_one(&mut self) -> Result<u8, StreamError<Self::IoError>> {
        let (&byte, rest) = self.slice.split_first().ok_or(StreamError::UnexpectedEof)?;
        self.slice = rest;
        Ok(byte)
    }
}

pub struct IterByteStream<I> {
    iter: I,
}

impl<I, T> IterByteStream<I>
where
    I: Iterator<Item = T>,
    T: IntoStreamResult,
{
    pub fn new(iter: I) -> Self {
        Self { iter }
    }

    pub fn inner(&self) -> &I {
        &self.iter
    }

    pub fn inner_mut(&mut self) -> &mut I {
        &mut self.iter
    }

    pub fn into_inner(self) -> I {
        self.iter
    }
}

impl<I, T> From<I> for IterByteStream<I>
where
    I: Iterator<Item = T>,
    T: IntoStreamResult,
{
    fn from(iter: I) -> Self {
        Self::new(iter)
    }
}

impl<I, T> ByteStream for IterByteStream<I>
where
    I: Iterator<Item = T>,
    T: IntoStreamResult,
{
    type IoError = T::IoError;

    fn read_n<const N: usize>(&mut self) -> Result<[u8; N], StreamError<Self::IoError>> {
        let mut buf = MaybeUninit::<[u8; N]>::uninit();
        let mut ptr = buf.as_mut_ptr() as *mut u8;

        for _ in 0..N {
            // No need to worry about dropping the initialised region of `buf` if the iterator panics
            // or returns an error, because `u8` does not require drop
            let byte = self.read_one()?;

            // SAFETY:
            // The resulting pointer is never more than 1 byte past the end of the array because we
            // increment it a maximum of `N` times. The pointer is only written to while it is within
            // the bounds of the array
            unsafe {
                ptr.write(byte);
                ptr = ptr.add(1);
            }
        }

        // SAFETY:
        // All `N` elements of `buf` are guaranteed to be initialised by this point. Therefore, `buf`
        // is initialised
        unsafe { Ok(buf.assume_init()) }
    }

    #[inline]
    fn read_one(&mut self) -> Result<u8, StreamError<Self::IoError>> {
        self.iter
            .next()
            .map(T::into_stream_result)
            .unwrap_or(Err(StreamError::UnexpectedEof))
    }
}

pub struct ReadByteStream<R> {
    reader: R,
}

impl<R> ReadByteStream<R>
where
    R: Read,
{
    pub fn new(reader: R) -> Self {
        Self { reader }
    }

    pub fn inner(&self) -> &R {
        &self.reader
    }

    pub fn inner_mut(&mut self) -> &mut R {
        &mut self.reader
    }

    pub fn into_inner(self) -> R {
        self.reader
    }
}

impl<R> From<R> for ReadByteStream<R>
where
    R: Read,
{
    fn from(reader: R) -> Self {
        Self::new(reader)
    }
}

impl<R> ByteStream for ReadByteStream<R>
where
    R: Read,
{
    type IoError = Box<io::Error>;

    fn read_n<const N: usize>(&mut self) -> Result<[u8; N], StreamError<Self::IoError>> {
        let mut buf = [0; N];
        self.reader
            .read_exact(&mut buf)
            .map(|_| buf)
            .map_err(|err| match err.kind() {
                io::ErrorKind::UnexpectedEof => StreamError::UnexpectedEof,
                _ => StreamError::Io(Box::new(err)),
            })
    }
}

pub trait IntoStreamResult: Sized {
    type IoError;

    fn into_stream_result(self) -> Result<u8, StreamError<Self::IoError>>;
}

impl IntoStreamResult for u8 {
    type IoError = Infallible;

    #[inline]
    fn into_stream_result(self) -> Result<u8, StreamError<Self::IoError>> {
        Ok(self)
    }
}

impl<E> IntoStreamResult for Result<u8, E> {
    type IoError = E;

    #[inline]
    fn into_stream_result(self) -> Result<u8, StreamError<Self::IoError>> {
        self.map_err(StreamError::Io)
    }
}

#[derive(Debug)]
pub enum StreamError<E> {
    UnexpectedEof,
    Io(E),
}

impl<E> fmt::Display for StreamError<E>
where
    E: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StreamError::UnexpectedEof => f.write_str("unexpected eof"),
            StreamError::Io(err) => err.fmt(f),
        }
    }
}

impl<E> error::Error for StreamError<E> where E: error::Error {}

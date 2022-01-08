// TODO
// [x] Decode
// [ ] Encode
// [ ] Image viewer
// [ ] no_std

pub mod byte_stream;
pub mod decode;
pub mod header;
mod hex;
pub mod pixel;
mod pixel_index;

pub use decode::Decoder;
pub use header::Header;
pub use pixel::Pixel;

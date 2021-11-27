// TODO
// [x] Decode
// [ ] Encode
// [ ] Image viewer

use std::env;
use std::fs;
use std::io;
use std::mem::{self, MaybeUninit};
use std::slice;

use image::ImageEncoder;

#[derive(Debug)]
struct Header {
    width: u16,
    height: u16,
    size: u32,
}

#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
struct Pixel {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

impl PartialEq for Pixel {
    fn eq(&self, other: &Self) -> bool {
        self.r == other.r && self.g == other.g && self.b == other.b && self.a == other.a
    }
}

impl Eq for Pixel {}

impl Pixel {
    const fn black() -> Self {
        Self {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        }
    }
}

struct Index {
    inner: [Pixel; Self::SIZE as usize],
}

impl Index {
    const SIZE: u8 = 64;

    const fn new() -> Self {
        Self {
            inner: [Pixel::black(); Self::SIZE as usize],
        }
    }

    fn pixel_hash(pixel: Pixel) -> u8 {
        (pixel.r ^ pixel.g ^ pixel.b ^ pixel.a) % Self::SIZE
    }

    fn get(&self, i: u8) -> Pixel {
        self.inner[i as usize]
    }

    fn put(&mut self, pixel: Pixel) {
        self.inner[Self::pixel_hash(pixel) as usize] = pixel;
    }

    // fn update(&mut self, pixel: Pixel) -> Option<u8> {
    //     let i = Self::pixel_hash(pixel);
    //     match self.inner[i as usize] == pixel {
    //         true => Some(i),
    //         false => {
    //             self.inner[i as usize] = pixel;
    //             None
    //         }
    //     }
    // }
}

struct Decoder<I> {
    iter: I,
}

impl<I> Decoder<I>
where
    I: Iterator<Item = u8>,
{
    const MAGIC: [u8; 4] = *b"qoif";

    fn new(iter: I) -> Self {
        Decoder {
            iter,
        }
    }

    fn parse_header(mut self) -> Option<(Header, PixelDecoder<I>)> {
        if take_bytes(&mut self.iter)? != Self::MAGIC {
            return None;
        }
    
        let header = Header {
            width: u16::from_be_bytes(take_bytes(&mut self.iter)?),
            height: u16::from_be_bytes(take_bytes(&mut self.iter)?),
            size: u32::from_be_bytes(take_bytes(&mut self.iter)?),
        };

        let pixel_decoder = PixelDecoder {
            iter: self.iter,
            index: Index::new(),
            prev: Pixel::black(),
            run: 0,
            remaining: (header.width as usize).checked_mul(header.height as usize),
        };

        Some((header, pixel_decoder))
    }
}

struct PixelDecoder<I> {
    iter: I,
    index: Index,
    prev: Pixel,
    run: u32,
    remaining: Option<usize>,
}

impl<I> PixelDecoder<I>
where
    I: Iterator<Item = u8>,
{
    const MASK_2: u8 = 0b11000000;
    const MASK_3: u8 = 0b11100000;
    const MASK_4: u8 = 0b11110000;

    const CHUNK_INDEX: u8 = 0b00_000000;
    const CHUNK_RUN_8: u8 = 0b010_00000;
    const CHUNK_RUN_16: u8 = 0b011_00000;
    const CHUNK_DIFF_8: u8 = 0b10_000000;
    const CHUNK_DIFF_16: u8 = 0b110_00000;
    const CHUNK_DIFF_24: u8 = 0b1110_0000;
    const CHUNK_COLOUR: u8 = 0b1111_0000;

    const COLOUR_R_FLAG: u8 = 0b00001000;
    const COLOUR_G_FLAG: u8 = 0b00000100;
    const COLOUR_B_FLAG: u8 = 0b00000010;
    const COLOUR_A_FLAG: u8 = 0b00000001;

    const DIFF_2_MASK: u8 = 0b00000011;
    const DIFF_4_MASK: u8 = 0b00001111;
    const DIFF_5_MASK: u8 = 0b00011111;

    fn next_byte(&mut self) -> Option<u8> {
        self.iter.next()
    }

    fn next_bytes<const N: usize>(&mut self) -> Option<[u8; N]> {
        take_bytes(&mut self.iter)
    }
}

//TODO: returning errors & stopping iterator on error state (e.g. set remaining to 0)
impl<I> Iterator for PixelDecoder<I>
where
    I: Iterator<Item = u8>,
{
    type Item = Pixel;

    fn next(&mut self) -> Option<Self::Item> {
        match self.remaining {
            Some(0) => return None,
            Some(n) => self.remaining = Some(n - 1),
            None => (),
        }

        if self.run > 0 {
            self.run -= 1;
            return Some(self.prev);
        }

        let b1 = self.next_byte()?;

        if b1 & Self::MASK_2 == Self::CHUNK_INDEX {
            self.prev = self.index.get(b1 & !Self::MASK_2);
        }
        else if b1 & Self::MASK_3 == Self::CHUNK_RUN_8 {
            self.run = (b1 & !Self::MASK_3) as u32;
        }
        else if b1 & Self::MASK_3 == Self::CHUNK_RUN_16 {
            let b2 = self.next_byte()?;
            self.run = ((((b1 & !Self::MASK_3) as u32) << 8) | b2 as u32) + 32;
        }
        else if b1 & Self::MASK_2 == Self::CHUNK_DIFF_8 {
            // 10RRGGBB
            self.prev.r = self.prev.r
                .wrapping_add(((b1 >> 4) & Self::DIFF_2_MASK).wrapping_sub(1));
            self.prev.g = self.prev.g
                .wrapping_add(((b1 >> 2) & Self::DIFF_2_MASK).wrapping_sub(1));
            self.prev.b = self.prev.b
                .wrapping_add((b1 & Self::DIFF_2_MASK).wrapping_sub(1));
            self.index.put(self.prev);
        }
        else if b1 & Self::MASK_3 == Self::CHUNK_DIFF_16 {
            // 110RRRRR GGGGBBBB
            let b2 = self.next_byte()?;
            self.prev.r = self.prev.r
                .wrapping_add((b1 & Self::DIFF_5_MASK).wrapping_sub(15));
            self.prev.g = self.prev.g
                .wrapping_add(((b2 >> 4) & Self::DIFF_4_MASK).wrapping_sub(7));
            self.prev.b = self.prev.b
                .wrapping_add((b2 & Self::DIFF_4_MASK).wrapping_sub(7));
            self.index.put(self.prev);
        }
        else if b1 & Self::MASK_4 == Self::CHUNK_DIFF_24 {
            // 1110RRRR RGGGGGBB BBBAAAAA
            let [b2, b3] = self.next_bytes()?;
            self.prev.r = self.prev.r
                .wrapping_add((((b1 << 1) & Self::DIFF_5_MASK) | (b2 >> 7)).wrapping_sub(15));
            self.prev.g = self.prev.g
                .wrapping_add(((b2 >> 2) & Self::DIFF_5_MASK).wrapping_sub(15));
            self.prev.b = self.prev.b
                .wrapping_add((((b2 << 3) & Self::DIFF_5_MASK) | (b3 >> 5)).wrapping_sub(15));
            self.prev.a = self.prev.a
                .wrapping_add((b3 & Self::DIFF_5_MASK).wrapping_sub(15));
            self.index.put(self.prev);
        }
        else if b1 & Self::MASK_4 == Self::CHUNK_COLOUR {
            if b1 & Self::COLOUR_R_FLAG != 0 {
                self.prev.r = self.next_byte()?;
            }
            if b1 & Self::COLOUR_G_FLAG != 0 {
                self.prev.g = self.next_byte()?;
            }
            if b1 & Self::COLOUR_B_FLAG != 0 {
                self.prev.b = self.next_byte()?;
            }
            if b1 & Self::COLOUR_A_FLAG != 0 {
                self.prev.a = self.next_byte()?;
            }
            self.index.put(self.prev);
        }

        Some(self.prev)
    }
    
    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, self.remaining)
    }
}

pub fn take_bytes<I, const N: usize>(iter: &mut I) -> Option<[u8; N]>
where
    I: Iterator<Item = u8>,
{
    let mut array = MaybeUninit::<[u8; N]>::uninit();
    let mut current_ptr = array.as_mut_ptr() as *mut u8;

    for _ in 0..N {
        // No need to worry about dropping the initialised region of `array` if iterator panics
        // or returns `None`, because `u8` does not require drop
        let b = iter.next()?;

        // SAFETY:
        // The resulting pointer is never more than 1 byte past the end of the array because we
        // increment it a maximum of `N` times. The pointer is only written to while it is within
        // the bounds of the array
        unsafe {
            current_ptr.write(b);
            current_ptr = current_ptr.add(1);
        }
    }

    // SAFETY:
    // All `N` elements of the array are guaranteed to be initialised by this point. Therefore,
    // the array is initialised
    unsafe { Some(array.assume_init()) }
}

fn main() {
    let args: Vec<String> = env::args().collect();

    let in_path = args.get(1)
        .expect("input file not specified");
    let out_path = args.get(2)
        .expect("output file not specified");

    let bytes = fs::read(in_path)
        .expect("failed to read file");

    let iter = bytes.iter().copied();

    let decoder = Decoder::new(iter);
    let (header, decoder) = decoder.parse_header()
        .expect("bad header");

    //TODO: check header "size" value against actual size (minus header's 12 bytes)

    let pixels = decoder.collect::<Vec<_>>();

    let pixels_raw = unsafe {
        slice::from_raw_parts(
            pixels.as_slice().as_ptr() as *const u8,
            pixels.len() * mem::size_of::<Pixel>()
        )
    };

    let out_f = fs::File::create(out_path)
        .expect("failed to create output file");
    let buf_writer = io::BufWriter::new(out_f);

    let encoder = image::codecs::png::PngEncoder::new(buf_writer);

    encoder.write_image(pixels_raw, header.width as u32, header.height as u32, image::ColorType::Rgba8)
        .expect("failed to write image");

    println!("Done!");
}

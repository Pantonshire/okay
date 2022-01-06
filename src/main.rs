// TODO
// [ ] Decode
// [ ] Encode
// [ ] Image viewer
// [ ] no_std

pub mod byte_stream;
pub mod decode;
pub mod header;
mod hex;
pub mod pixel;
mod pixel_index;

use std::env;
use std::fs;
use std::io::BufWriter;
// use std::fs::File;
// use std::io::BufReader;

pub use decode::Decoder;
pub use header::Header;
pub use pixel::Pixel;

use image::ImageEncoder;

fn main() {
    // let file = File::open("encoded.qoi").unwrap();
    // let reader = BufReader::new(file);
    // let (header, decoder) = Decoder::new_from_reader(reader).decode_header().unwrap();

    let args: Vec<String> = env::args().collect();

    let in_path = args.get(1)
        .expect("input file not specified");

    let out_path = args.get(2)
        .expect("output file not specified");

    let bytes = fs::read(&in_path).unwrap();
    let (header, decoder) = Decoder::new_from_iter(bytes).decode_header().unwrap();
    
    println!("{:?}", header);

    let rgba = decoder.decode_bytes_all(Pixel::rgba).unwrap();

    let out_file = fs::File::create(out_path).unwrap();
    let buf_writer = BufWriter::new(out_file);

    let encoder = image::codecs::png::PngEncoder::new(buf_writer);

    encoder.write_image(&rgba, header.width as u32, header.height as u32, image::ColorType::Rgba8)
        .unwrap();

    println!("Done!");
}

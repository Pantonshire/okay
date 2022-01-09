use std::env;
use std::fs;
use std::io::BufWriter;

use image::ImageEncoder;

use okay::{Decoder, Pixel};

fn main() {
    let args: Vec<String> = env::args().collect();

    let in_path = args.get(1)
        .expect("input file not specified");

    let out_path = args.get(2)
        .expect("output file not specified");

    let bytes = fs::read(&in_path).unwrap();
    let (header, decoder) = Decoder::new_from_iter(bytes).decode_header().unwrap();
    
    println!("{:?}", header);

    let rgba = decoder.decode_bytes_vec(Pixel::rgba).unwrap();

    let out_file = fs::File::create(out_path).unwrap();
    let buf_writer = BufWriter::new(out_file);

    let encoder = image::codecs::png::PngEncoder::new(buf_writer);

    encoder.write_image(&rgba, header.width() as u32, header.height() as u32, image::ColorType::Rgba8)
        .unwrap();

    println!("Done!");
}
#![feature(test)]

extern crate test;

use std::fs::{self, File};
use std::io::BufReader;

use test::{Bencher, black_box};

#[bench]
fn bench_pixels_slice(b: &mut Bencher) {
    b.iter(|| {
        let bytes = fs::read("multibot.qoi").unwrap();
        let (_header, decoder) = okay::Decoder::new_from_slice(&bytes).decode_header().unwrap();
        black_box(decoder.decode_pixels_all().unwrap());
    });
}

#[bench]
fn bench_pixels_iter(b: &mut Bencher) {
    b.iter(|| {
        let bytes = fs::read("multibot.qoi").unwrap();
        let (_header, decoder) = okay::Decoder::new_from_iter(bytes).decode_header().unwrap();
        black_box(decoder.decode_pixels_all().unwrap());
    });
}

#[bench]
fn bench_pixels_read(b: &mut Bencher) {
    b.iter(|| {
        let file = File::open("multibot.qoi").unwrap();
        let buf_reader = BufReader::new(file);
        let (_header, decoder) = okay::Decoder::new_from_reader(buf_reader).decode_header().unwrap();
        black_box(decoder.decode_pixels_all().unwrap());
    });
}

#[bench]
fn bench_bytes_read(b: &mut Bencher) {
    b.iter(|| {
        let file = File::open("multibot.qoi").unwrap();
        let buf_reader = BufReader::new(file);
        let (_header, decoder) = okay::Decoder::new_from_reader(buf_reader).decode_header().unwrap();
        black_box(decoder.decode_bytes_all(okay::Pixel::rgba).unwrap());
    });
}

#[bench]
fn bench_qoi_bytes_stream(b: &mut Bencher) {
    b.iter(|| {
        let file = File::open("multibot.qoi").unwrap();
        let buf_reader = BufReader::new(file);
        let mut decoder = qoi::Decoder::from_stream(buf_reader).unwrap();
        black_box(decoder.decode_to_vec().unwrap());
    });
}

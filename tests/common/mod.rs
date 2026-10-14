#![allow(dead_code)]
#![allow(
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]

pub mod block_decoder;

use std::path::{Path, PathBuf};

use leake::format::{Quality, Sampling, VtfFormat};
use leake::frame::Frame;
use leake::options::SprayOptions;

pub fn noisy_frame(width: u32, height: u32) -> Frame {
    let mut rgba = vec![0u8; (width * height * 4) as usize];
    let mut seed: i32 = 12_345;
    for y in 0..height {
        for x in 0..width {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            let noise = (seed as u32 >> 16) & 0x1f;
            let base = ((y * width + x) * 4) as usize;
            rgba[base] = (x as i32 * 255 / width as i32 + noise as i32) as u8;
            rgba[base + 1] = (y as i32 * 255 / height as i32 + noise as i32) as u8;
            rgba[base + 2] = ((x + y) as i32 * 255 / (width + height) as i32 + noise as i32) as u8;
            rgba[base + 3] = 0xff;
        }
    }
    Frame::new(width, height, rgba)
}

pub fn gradient_frame(width: u32, height: u32) -> Frame {
    let mut rgba = vec![0u8; (width * height * 4) as usize];
    for y in 0..height {
        for x in 0..width {
            let base = ((y * width + x) * 4) as usize;
            rgba[base] = (x * 255 / width) as u8;
            rgba[base + 1] = (y * 255 / height) as u8;
            rgba[base + 2] = ((x + y) * 255 / (width + height)) as u8;
            rgba[base + 3] = 0xff;
        }
    }
    Frame::new(width, height, rgba)
}

pub fn mid_alpha_frame(width: u32, height: u32) -> Frame {
    let mut rgba = vec![0u8; (width * height * 4) as usize];
    for y in 0..height {
        for x in 0..width {
            let base = ((y * width + x) * 4) as usize;
            rgba[base] = ((x * 16 + y) & 0xff) as u8;
            rgba[base + 1] = ((y * 16 + x) & 0xff) as u8;
            rgba[base + 2] = (((x + y) * 8) & 0xff) as u8;
            rgba[base + 3] = (64 + ((x * 7 + y * 11) % 128)) as u8;
        }
    }
    Frame::new(width, height, rgba)
}

pub fn half_clear_frame(width: u32, height: u32) -> Frame {
    let mut rgba = vec![0u8; (width * height * 4) as usize];
    for y in 0..height {
        for x in 0..width {
            let base = ((y * width + x) * 4) as usize;
            rgba[base] = ((x * 9 + 30) & 0xff) as u8;
            rgba[base + 1] = ((y * 5 + 90) & 0xff) as u8;
            rgba[base + 2] = (((x + y) * 4) & 0xff) as u8;
            rgba[base + 3] = if (x + y) % 2 == 0 { 0xff } else { 0 };
        }
    }
    Frame::new(width, height, rgba)
}

pub fn transparent_frame(width: u32, height: u32) -> Frame {
    Frame::new(width, height, vec![0u8; (width * height * 4) as usize])
}

pub fn flat_frame(width: u32, height: u32, red: u8, green: u8, blue: u8, alpha: u8) -> Frame {
    let mut rgba = vec![0u8; (width * height * 4) as usize];
    for pixel in 0..(width * height) as usize {
        let base = pixel * 4;
        rgba[base] = red;
        rgba[base + 1] = green;
        rgba[base + 2] = blue;
        rgba[base + 3] = alpha;
    }
    Frame::new(width, height, rgba)
}

pub fn corpus() -> Vec<(&'static str, Frame)> {
    vec![
        ("noisy1x1", noisy_frame(1, 1)),
        ("noisy5x5", noisy_frame(5, 5)),
        ("noisy8x8", noisy_frame(8, 8)),
        ("noisy20x12", noisy_frame(20, 12)),
        ("noisy64x64", noisy_frame(64, 64)),
        ("gradient64x64", gradient_frame(64, 64)),
        ("midAlpha8x8", mid_alpha_frame(8, 8)),
        ("halfClear64x64", half_clear_frame(64, 64)),
        ("clear8x8", transparent_frame(8, 8)),
        ("clear64x64", transparent_frame(64, 64)),
        ("flat64x64", flat_frame(64, 64, 200, 120, 40, 255)),
    ]
}

pub fn corpus64() -> Vec<(&'static str, Frame)> {
    corpus()
        .into_iter()
        .filter(|(_, frame)| frame.width == 64 && frame.height == 64)
        .collect()
}

pub fn options(inputs: Vec<PathBuf>) -> SprayOptions {
    SprayOptions {
        inputs,
        format: VtfFormat::Dxt1,
        quality: Quality::Slow,
        sampling: Sampling::Default,
        jobs: 2,
        ..SprayOptions::default()
    }
}

pub fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

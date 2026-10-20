#![allow(
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]

mod common;

use std::sync::OnceLock;

use common::block_decoder::{decode_dxt1, decode_dxt5};
use common::{gradient_frame, half_clear_frame, noisy_frame, transparent_frame};
use leake::encode;
use leake::format::{Quality, VtfFormat};
use leake::frame::Frame;
use leake::gpu::Encoder;

fn encoder() -> Option<&'static Encoder> {
    static ENCODER: OnceLock<Option<Encoder>> = OnceLock::new();
    ENCODER.get_or_init(|| Encoder::new().ok()).as_ref()
}

#[test]
fn packed_formats_match_the_cpu_encoder() {
    let Some(encoder) = encoder() else {
        return;
    };
    let raster = noisy_frame(37, 19);
    let formats = [
        VtfFormat::Rgba8888,
        VtfFormat::Rgb888,
        VtfFormat::Rgb565,
        VtfFormat::Bgra4444,
        VtfFormat::Bgra5551,
    ];
    for format in formats {
        for dither in [false, true] {
            let cpu = encode::encode(&raster, format, Quality::Slow, dither, 1, None);
            let gpu = encoder
                .encode(&raster, format, Quality::Slow, dither)
                .expect("GPU encode");
            if cpu != gpu {
                let index = cpu
                    .iter()
                    .zip(&gpu)
                    .position(|(left, right)| left != right)
                    .expect("different lengths");
                let bytes_per_pixel = packed_bytes_per_pixel(format);
                let pixel_index = index / bytes_per_pixel;
                let source = &raster.rgba[pixel_index * 4..pixel_index * 4 + 4];
                panic!(
                    "{format}, dither={dither}: byte {index} (pixel {pixel_index}, source {source:?}) was {} on the CPU and {} on the GPU",
                    cpu[index], gpu[index]
                );
            }
        }
    }
}

#[test]
fn packed_formats_keep_odd_dimensions() {
    let Some(encoder) = encoder() else {
        return;
    };
    let raster = gradient_frame(9, 5);
    for format in [VtfFormat::Rgb888, VtfFormat::Bgra4444] {
        let bytes = encoder
            .encode(&raster, format, Quality::Slow, true)
            .expect("GPU encode");
        assert_eq!(
            raster.rgba.len() / 4 * packed_bytes_per_pixel(format),
            bytes.len()
        );
    }
}

#[test]
fn dxt1_decodes_close_to_the_source() {
    let Some(encoder) = encoder() else {
        return;
    };
    let raster = gradient_frame(64, 64);
    let data = encoder
        .encode(&raster, VtfFormat::Dxt1, Quality::Slow, true)
        .expect("GPU encode");
    assert_eq!(64 * 64 / 2, data.len());
    let decoded = decode_dxt1(&data, 0, 16, 64, 64);
    let (average, worst) = colour_error(&raster, &decoded);
    assert!(average < 8.0, "average colour error was {average}");
    assert!(worst < 48, "worst colour error was {worst}");
}

#[test]
fn dxt5_keeps_alpha_within_reach() {
    let Some(encoder) = encoder() else {
        return;
    };
    let raster = smooth_alpha_frame(48, 48);
    let data = encoder
        .encode(&raster, VtfFormat::Dxt5, Quality::Slow, true)
        .expect("GPU encode");
    assert_eq!(48 * 48, data.len());
    let decoded = decode_dxt5(&data, 0, 12, 48, 48);
    let mut alpha_error = 0u64;
    for (source, result) in raster.rgba.as_chunks::<4>().0.iter().zip(&decoded) {
        alpha_error += u64::from(source[3].abs_diff((result >> 24) as u8));
    }
    let average = alpha_error as f64 / f64::from(48 * 48);
    assert!(average < 12.0, "average alpha error was {average}");
    let (colour_average, colour_worst) = colour_error(&raster, &decoded);
    assert!(
        colour_average < 12.0,
        "average colour error was {colour_average}"
    );
    assert!(colour_worst < 64, "worst colour error was {colour_worst}");
}

#[test]
fn dxt1_keeps_transparency() {
    let Some(encoder) = encoder() else {
        return;
    };
    let raster = half_clear_frame(32, 32);
    let data = encoder
        .encode(&raster, VtfFormat::Dxt1, Quality::Slow, true)
        .expect("GPU encode");
    let decoded = decode_dxt1(&data, 0, 8, 32, 32);
    for (index, (source, result)) in raster
        .rgba
        .as_chunks::<4>()
        .0
        .iter()
        .zip(&decoded)
        .enumerate()
    {
        let expected = if source[3] < 127 { 0 } else { 255 };
        assert_eq!(
            expected,
            (result >> 24) as u8,
            "pixel {index} had alpha {} in the source",
            source[3]
        );
    }
}

#[test]
fn a_fully_transparent_block_is_encoded() {
    let Some(encoder) = encoder() else {
        return;
    };
    let raster = transparent_frame(8, 8);
    let data = encoder
        .encode(&raster, VtfFormat::Dxt1, Quality::Slow, true)
        .expect("GPU encode");
    let decoded = decode_dxt1(&data, 0, 2, 8, 8);
    assert!(decoded.iter().all(|pixel| (pixel >> 24) == 0));
}

#[test]
fn the_encoder_handles_dimensions_that_are_not_block_aligned() {
    let Some(encoder) = encoder() else {
        return;
    };
    let raster = noisy_frame(10, 6);
    let dxt1 = encoder
        .encode(&raster, VtfFormat::Dxt1, Quality::Normal, true)
        .expect("GPU encode");
    assert_eq!(3 * 2 * 8, dxt1.len());
    let dxt5 = encoder
        .encode(&raster, VtfFormat::Dxt5, Quality::Normal, true)
        .expect("GPU encode");
    assert_eq!(3 * 2 * 16, dxt5.len());
    assert_eq!(60, decode_dxt5(&dxt5, 0, 3, 10, 6).len());
}

fn packed_bytes_per_pixel(format: VtfFormat) -> usize {
    match format {
        VtfFormat::Rgba8888 => 4,
        VtfFormat::Rgb888 => 3,
        _ => 2,
    }
}

fn smooth_alpha_frame(width: u32, height: u32) -> Frame {
    let mut rgba = vec![0u8; (width * height * 4) as usize];
    for y in 0..height {
        for x in 0..width {
            let base = ((y * width + x) * 4) as usize;
            rgba[base] = (x * 255 / width) as u8;
            rgba[base + 1] = (y * 255 / height) as u8;
            rgba[base + 2] = ((x + y) * 255 / (width + height)) as u8;
            rgba[base + 3] = (64 + (x * 3 + y * 5) % 128) as u8;
        }
    }
    Frame::new(width, height, rgba)
}

fn colour_error(source: &Frame, decoded: &[u32]) -> (f64, u8) {
    let mut total = 0u64;
    let mut worst = 0u8;
    let mut count = 0u64;
    for (pixel, result) in source.rgba.as_chunks::<4>().0.iter().zip(decoded) {
        let channels = [
            (pixel[0], (result >> 16) as u8),
            (pixel[1], (result >> 8) as u8),
            (pixel[2], *result as u8),
        ];
        for (source_channel, result_channel) in channels {
            let error = source_channel.abs_diff(result_channel);
            total += u64::from(error);
            worst = worst.max(error);
            count += 1;
        }
    }
    (total as f64 / count as f64, worst)
}

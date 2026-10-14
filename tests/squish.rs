#![allow(
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]

mod common;

use common::block_decoder::{decode_alpha_block, decode_dxt1, decode_dxt5};
use common::gradient_frame;
use leake::encode;
use leake::format::{Quality, VtfFormat};
use leake::frame::Frame;
use leake::squish;

const QUALITIES: [Quality; 4] = [
    Quality::Fast,
    Quality::Normal,
    Quality::Slow,
    Quality::VerySlow,
];

#[test]
fn flat_blocks_survive_the_round_trip_exactly() {
    let colours = [
        [0u8, 0, 0],
        [255, 255, 255],
        [255, 0, 0],
        [0, 255, 0],
        [0, 0, 255],
        [255, 255, 0],
    ];
    for colour in colours {
        let block = [argb(
            u32::from(colour[0]),
            u32::from(colour[1]),
            u32::from(colour[2]),
            255,
        ); 16];
        let decoded = decode_dxt1(&encode_dxt1(&block, Quality::Slow), 0, 1, 4, 4);
        for pixel in decoded {
            assert_eq!(
                [colour[0], colour[1], colour[2], 255],
                channels(pixel),
                "a flat block of {colour:?} did not survive the round trip"
            );
        }
    }
}

#[test]
fn a_smooth_gradient_decodes_close_to_its_source() {
    let mut block = [0i32; 16];
    for (index, value) in block.iter_mut().enumerate() {
        let x = (index % 4) as u32;
        let y = (index / 4) as u32;
        *value = argb(100 + x * 4, 128 + y * 3, 60 + (x + y) * 2, 255);
    }
    let decoded = decode_dxt1(&encode_dxt1(&block, Quality::Slow), 0, 1, 4, 4);
    for (index, source) in block.iter().enumerate() {
        let source = channels(*source as u32);
        let result = channels(decoded[index]);
        for channel in 0..3 {
            let difference = i32::from(source[channel]) - i32::from(result[channel]);
            assert!(
                difference.abs() <= 12,
                "pixel {index} channel {channel} was {}, expected about {}",
                result[channel],
                source[channel]
            );
        }
    }
}

#[test]
fn transparent_pixels_punch_through_in_dxt1() {
    let mut block = [0i32; 16];
    for (index, value) in block.iter_mut().enumerate() {
        *value = if index % 2 == 0 {
            argb(200, 40, 40, 255)
        } else {
            argb(0, 0, 0, 0)
        };
    }
    let decoded = decode_dxt1(&encode_dxt1(&block, Quality::Slow), 0, 1, 4, 4);
    for index in (0..16).step_by(2) {
        assert_eq!(
            255,
            channels(decoded[index])[3],
            "the opaque pixel at {index} should have stayed opaque"
        );
        assert_eq!(
            0,
            channels(decoded[index + 1])[3],
            "the clear pixel at {index} should have punched through"
        );
    }
}

#[test]
fn dxt5_keeps_alpha_within_its_quantisation_bounds() {
    let alpha = [
        0, 17, 34, 51, 68, 85, 102, 119, 136, 153, 170, 187, 204, 221, 238, 255,
    ];
    let mut output = [0i32; 2];
    squish::compress_alpha_block(&alpha, &Quality::Slow.config(), &mut output);
    let mut bytes = vec![0u8; 8];
    write_dwords(&mut bytes, &output);
    let decoded = decode_alpha_block(&bytes, 0);
    for (index, expected) in alpha.iter().enumerate() {
        let difference = decoded[index] as i32 - *expected;
        assert!(
            difference.abs() <= 20,
            "alpha {index} decoded as {}, expected about {expected}",
            decoded[index]
        );
    }
}

#[test]
fn a_block_encodes_the_same_whatever_came_before_it() {
    let mut target = [0i32; 16];
    let mut other = [0i32; 16];
    for (index, value) in target.iter_mut().enumerate() {
        *value = argb(
            (index * 7) as u32 & 0xff,
            (index * 13) as u32 & 0xff,
            (index * 29) as u32 & 0xff,
            255,
        );
    }
    for (index, value) in other.iter_mut().enumerate() {
        *value = argb((index * 31) as u32 & 0xff, 12, 200, 255);
    }
    let alone = encode_dxt1(&target, Quality::Slow);
    let _ = encode_dxt1(&other, Quality::Slow);
    assert_eq!(
        alone,
        encode_dxt1(&target, Quality::Slow),
        "compressing another block first changed the result"
    );
}

#[test]
fn dxt1_keeps_transparent_pixels_transparent_at_every_quality() {
    for quality in QUALITIES {
        let mut block = [0i32; 16];
        for (index, value) in block.iter_mut().enumerate() {
            *value = if index % 2 == 0 {
                argb(220, 30, 30, 255)
            } else {
                argb(0, 0, 0, 0)
            };
        }
        let decoded = decode_dxt1(&encode_dxt1(&block, quality), 0, 1, 4, 4);
        for index in (0..16).step_by(2) {
            assert_eq!(
                255,
                channels(decoded[index])[3],
                "opaque pixel at {index} was lost at {quality}"
            );
            assert_eq!(
                0,
                channels(decoded[index + 1])[3],
                "clear pixel at {index} stayed opaque at {quality}"
            );
        }
    }
}

#[test]
fn a_fully_transparent_block_stays_transparent_at_every_quality() {
    for quality in QUALITIES {
        let decoded = decode_dxt1(&encode_dxt1(&[0i32; 16], quality), 0, 1, 4, 4);
        for pixel in decoded {
            assert_eq!(
                0,
                channels(pixel)[3],
                "an empty block came back opaque at {quality}"
            );
        }
    }
}

#[test]
fn dxt5_keeps_alpha_within_its_quantisation_bounds_at_every_quality() {
    let alpha = [
        0, 32, 64, 96, 128, 160, 192, 224, 8, 40, 72, 104, 136, 168, 200, 232,
    ];
    for quality in QUALITIES {
        let mut output = [0i32; 2];
        squish::compress_alpha_block(&alpha, &quality.config(), &mut output);
        let mut bytes = vec![0u8; 8];
        write_dwords(&mut bytes, &output);
        let decoded = decode_alpha_block(&bytes, 0);
        for (index, expected) in alpha.iter().enumerate() {
            let difference = decoded[index] as i32 - *expected;
            assert!(
                difference.abs() <= 20,
                "alpha {index} decoded as {} at {quality}, expected about {expected}",
                decoded[index]
            );
        }
    }
}

#[test]
fn a_dxt5_level_carries_colour_and_alpha_side_by_side() {
    let mut rgba = vec![0u8; 8 * 8 * 4];
    for y in 0..8u32 {
        for x in 0..8u32 {
            let base = ((y * 8 + x) * 4) as usize;
            let clear = x >= 4;
            rgba[base] = if clear { 0 } else { 240 };
            rgba[base + 1] = 40;
            rgba[base + 2] = if clear { 240 } else { 0 };
            rgba[base + 3] = if clear { 130 } else { 255 };
        }
    }
    let encoded = encode_level(&Frame::new(8, 8, rgba), VtfFormat::Dxt5, 1);
    let decoded = decode_dxt5(&encoded, 0, 2, 8, 8);

    let pixel = |x: usize, y: usize| channels(decoded[y * 8 + x]);

    assert_eq!(
        255,
        pixel(1, 1)[3],
        "the opaque half should have stayed opaque"
    );
    assert!(pixel(1, 1)[0] > 200, "the opaque half should still be red");
    assert!(
        (i32::from(pixel(5, 1)[3]) - 130).abs() <= 8,
        "the clear half decoded as {}",
        pixel(5, 1)[3]
    );
    assert!(pixel(5, 1)[2] > 200, "the clear half should still be blue");
}

#[test]
fn a_raster_of_only_clear_pixels_encodes_without_complaint() {
    let encoded = encode_level(&Frame::new(8, 8, vec![0u8; 8 * 8 * 4]), VtfFormat::Dxt1, 2);
    let decoded = decode_dxt1(&encoded, 0, 2, 8, 8);
    for pixel in decoded {
        assert_eq!(0, channels(pixel)[3]);
    }
}

#[test]
fn parallel_encoding_produces_the_same_bytes_as_serial_encoding() {
    let raster = gradient_frame(64, 64);
    let serial = encode_level(&raster, VtfFormat::Dxt1, 1);
    let parallel = encode_level(&raster, VtfFormat::Dxt1, 8);
    assert_eq!(serial, parallel);
}

#[test]
fn dxt1_blocks_are_eight_bytes_and_dxt5_blocks_are_sixteen() {
    let raster = gradient_frame(8, 8);
    assert_eq!(4 * 8, encode_level(&raster, VtfFormat::Dxt1, 2).len());
    assert_eq!(4 * 16, encode_level(&raster, VtfFormat::Dxt5, 2).len());
}

#[test]
fn pixels_past_the_edge_of_a_texture_are_padded_clear() {
    let raster = gradient_frame(5, 5);
    let encoded = encode_level(&raster, VtfFormat::Dxt1, 2);
    let decoded = decode_dxt1(&encoded, 0, 2, 8, 8);
    assert_eq!(
        255,
        channels(decoded[4 * 8 + 4])[3],
        "the last real pixel should be opaque"
    );
    assert_eq!(
        0,
        channels(decoded[5 * 8 + 5])[3],
        "padding should be transparent"
    );
    assert_eq!(
        0,
        channels(decoded[7 * 8 + 7])[3],
        "padding should be transparent"
    );
}

fn encode_dxt1(block: &[i32; 16], quality: Quality) -> Vec<u8> {
    let mut output = [0i32; 2];
    squish::compress_rgb_block(block, &quality.config(), true, 127, &mut output);
    let mut bytes = vec![0u8; 8];
    write_dwords(&mut bytes, &output);
    bytes
}

fn encode_level(raster: &Frame, format: VtfFormat, jobs: usize) -> Vec<u8> {
    encode::encode(raster, format, Quality::Slow, true, jobs, None)
}

fn write_dwords(target: &mut [u8], dwords: &[i32]) {
    for (index, dword) in dwords.iter().enumerate() {
        target[index * 4..index * 4 + 4].copy_from_slice(&dword.to_le_bytes());
    }
}

fn argb(red: u32, green: u32, blue: u32, alpha: u32) -> i32 {
    ((alpha << 24) | (red << 16) | (green << 8) | blue) as i32
}

fn channels(pixel: u32) -> [u8; 4] {
    [
        (pixel >> 16) as u8,
        (pixel >> 8) as u8,
        pixel as u8,
        (pixel >> 24) as u8,
    ]
}

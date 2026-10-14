#![allow(
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]

mod common;

use std::collections::HashMap;

use leake::encode;
use leake::format::{Quality, VtfFormat};
use leake::frame::Frame;
use leake::quantise::{self, FormatSpec};

const LADDER_STEPS: usize = 17;
const DITHER_SIZE: usize = 4;
const TIE_TRIPLES: usize = 48;

const PACKED_FORMATS: [VtfFormat; 5] = [
    VtfFormat::Rgba8888,
    VtfFormat::Rgb888,
    VtfFormat::Rgb565,
    VtfFormat::Bgra5551,
    VtfFormat::Bgra4444,
];

const DITHER_MATRIX: [[u8; DITHER_SIZE * DITHER_SIZE]; LADDER_STEPS] = [
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0],
    [1, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0],
    [1, 0, 1, 0, 0, 0, 0, 0, 1, 0, 1, 0, 0, 0, 0, 0],
    [1, 0, 1, 0, 0, 1, 0, 0, 1, 0, 1, 0, 0, 0, 0, 0],
    [1, 0, 1, 0, 0, 1, 0, 0, 1, 0, 1, 0, 0, 0, 0, 1],
    [1, 0, 1, 0, 0, 1, 0, 1, 1, 0, 1, 0, 0, 0, 0, 1],
    [1, 0, 1, 0, 0, 1, 0, 1, 1, 0, 1, 0, 0, 1, 0, 1],
    [1, 1, 1, 0, 0, 1, 0, 1, 1, 0, 1, 0, 0, 1, 0, 1],
    [1, 1, 1, 0, 0, 1, 0, 1, 1, 0, 1, 1, 0, 1, 0, 1],
    [1, 1, 1, 1, 0, 1, 0, 1, 1, 0, 1, 1, 0, 1, 0, 1],
    [1, 1, 1, 1, 0, 1, 0, 1, 1, 1, 1, 1, 0, 1, 0, 1],
    [1, 1, 1, 1, 1, 1, 0, 1, 1, 1, 1, 1, 0, 1, 0, 1],
    [1, 1, 1, 1, 1, 1, 0, 1, 1, 1, 1, 1, 0, 1, 1, 1],
    [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 1, 1, 1],
    [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
];

#[test]
fn rgb565_channels_stay_on_one_of_the_two_surrounding_steps() {
    let raster = gradient(32, 32);
    let bytes = encode_raster(&raster, VtfFormat::Rgb565, true);
    for y in 0..raster.height {
        for x in 0..raster.width {
            let pixel = little_endian16(&bytes, ((y * raster.width + x) * 2) as usize);
            let source = raster.pixel(x, y);
            assert_on_step(
                i32::from(source[0]),
                pixel & 0x1f,
                5,
                &format!("red at {x},{y}"),
            );
            assert_on_step(
                i32::from(source[1]),
                (pixel >> 5) & 0x3f,
                6,
                &format!("green at {x},{y}"),
            );
            assert_on_step(
                i32::from(source[2]),
                (pixel >> 11) & 0x1f,
                5,
                &format!("blue at {x},{y}"),
            );
        }
    }
}

#[test]
fn rgb565_round_trips_within_half_a_step() {
    for value in 0..=255u8 {
        let bytes = encode_raster(&pixel(value, value, value, 255), VtfFormat::Rgb565, false);
        let packed = little_endian16(&bytes, 0);
        let red = expand(packed & 0x1f, 3);
        let green = expand((packed >> 5) & 0x3f, 2);
        let blue = expand((packed >> 11) & 0x1f, 3);
        assert!(
            (red - i32::from(value)).abs() <= 6,
            "{value} decoded as {red}"
        );
        assert!(
            (green - i32::from(value)).abs() <= 6,
            "{value} decoded as {green}"
        );
        assert!(
            (blue - i32::from(value)).abs() <= 6,
            "{value} decoded as {blue}"
        );
    }
}

#[test]
fn bgra5551_writes_a_real_word_and_snaps_alpha_to_one_bit() {
    let opaque = encode_raster(&pixel(200, 60, 20, 255), VtfFormat::Bgra5551, false);
    let word = little_endian16(&opaque, 0);
    assert_ne!(0, word, "an opaque pixel must not pack to zero");
    assert_eq!(1, (word >> 15) & 0x1);

    let nearly_opaque = little_endian16(
        &encode_raster(&pixel(200, 60, 20, 200), VtfFormat::Bgra5551, false),
        0,
    );
    assert_eq!(
        1,
        (nearly_opaque >> 15) & 0x1,
        "alpha above the midpoint should be fully opaque"
    );
    let mostly_clear = little_endian16(
        &encode_raster(&pixel(200, 60, 20, 100), VtfFormat::Bgra5551, false),
        0,
    );
    assert_eq!(
        0,
        (mostly_clear >> 15) & 0x1,
        "alpha below the midpoint should be fully clear"
    );
}

#[test]
fn bgra4444_snaps_alpha_to_one_of_sixteen_steps() {
    for alpha in 0..=255u8 {
        let word = little_endian16(
            &encode_raster(&pixel(10, 20, 30, alpha), VtfFormat::Bgra4444, false),
            0,
        );
        let step = (word >> 12) & 0xf;
        assert!(
            (step as i32 * 17 - i32::from(alpha)).abs() <= 9,
            "alpha {alpha} landed on step {step}"
        );
    }
}

#[test]
fn rgb888_drops_alpha_and_rgba8888_keeps_it() {
    let three = encode_raster(&pixel(10, 20, 30, 40), VtfFormat::Rgb888, false);
    assert_eq!(3, three.len());
    assert_eq!(10, three[0]);
    assert_eq!(20, three[1]);
    assert_eq!(30, three[2]);

    let four = encode_raster(&pixel(10, 20, 30, 40), VtfFormat::Rgba8888, false);
    assert_eq!(4, four.len());
    assert_eq!(40, four[3]);
}

#[test]
fn dithering_changes_the_result_but_stays_inside_the_step_pair() {
    let raster = gradient(16, 16);
    let dithered = encode_raster(&raster, VtfFormat::Rgb565, true);
    let plain = encode_raster(&raster, VtfFormat::Rgb565, false);
    assert_ne!(
        dithered, plain,
        "dithering should change at least one pixel"
    );

    for y in 0..raster.height {
        for x in 0..raster.width {
            let source = raster.pixel(x, y)[0];
            let value = little_endian16(&dithered, ((y * raster.width + x) * 2) as usize) & 0x1f;
            assert_on_step(i32::from(source), value, 5, &format!("red at {x},{y}"));
        }
    }
}

#[test]
fn reduce_matches_the_reference_formulation_across_every_value() {
    for format in PACKED_FORMATS {
        for dither in [false, true] {
            let mut reducer = quantise::reducer(format, dither);
            for channel in 0..4 {
                for value in 0..=255 {
                    let source = swept(channel, value);
                    for y in 0..4 {
                        for x in 0..4 {
                            assert_eq!(
                                reference_reduce(source, format, dither, x, y),
                                reducer.reduce(source, x, y),
                                "channel {channel} value {value} in {format} dither={dither} at {x},{y}"
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn colour_triples_equidistant_between_two_shades_match_the_reference() {
    for format in PACKED_FORMATS {
        let spec = quantise::spec(format);
        if spec.true_colour() {
            continue;
        }
        let triples = tied_triples(spec);
        assert!(!triples.is_empty(), "{format} has no triples to tie on");
        for dither in [false, true] {
            let mut reducer = quantise::reducer(format, dither);
            for triple in &triples {
                let source = argb(triple[0] as u32, triple[1] as u32, triple[2] as u32, 137);
                for y in 0..4 {
                    for x in 0..4 {
                        assert_eq!(
                            reference_reduce(source, format, dither, x, y),
                            reducer.reduce(source, x, y),
                            "tie {triple:?} in {format} dither={dither} at {x},{y}"
                        );
                    }
                }
            }
        }
    }
}

fn swept(channel: usize, value: u8) -> u32 {
    let mut channels = [37u32, 91, 173, 205];
    channels[channel] = u32::from(value);
    argb(channels[0], channels[1], channels[2], channels[3])
}

fn argb(red: u32, green: u32, blue: u32, alpha: u32) -> u32 {
    (alpha << 24) | (red << 16) | (green << 8) | blue
}

fn reference_reduce(pixel: u32, format: VtfFormat, dither: bool, x: u32, y: u32) -> u32 {
    let spec = quantise::spec(format);
    if spec.identity() {
        return pixel;
    }
    let alpha = if spec.opaque {
        255
    } else {
        snap_alpha(((pixel >> 24) & 0xff) as i32, spec.alpha)
    };
    let red = ((pixel >> 16) & 0xff) as i32;
    let green = ((pixel >> 8) & 0xff) as i32;
    let blue = (pixel & 0xff) as i32;
    let colour = if spec.true_colour() {
        ((red as u32) << 16) | ((green as u32) << 8) | blue as u32
    } else {
        reference_colour(red, green, blue, spec, dither, x, y)
    };
    ((alpha as u32) << 24) | colour
}

fn reference_colour(
    red: i32,
    green: i32,
    blue: i32,
    spec: FormatSpec,
    dither: bool,
    x: u32,
    y: u32,
) -> u32 {
    let source = [f64::from(red), f64::from(green), f64::from(blue)];
    let lower = [
        lower_shade(red, spec.red),
        lower_shade(green, spec.green),
        lower_shade(blue, spec.blue),
    ];
    let upper = [
        upper_shade(red, spec.red),
        upper_shade(green, spec.green),
        upper_shade(blue, spec.blue),
    ];
    let near_is_lower = distance(&lower, &source) <= distance(&upper, &source);
    let near = if near_is_lower { lower } else { upper };
    let far = if near_is_lower { upper } else { lower };
    if !dither || !spec.dithers {
        return pack(&near);
    }

    let mut rung = 0;
    let mut closest = f64::MAX;
    for candidate in 0..LADDER_STEPS {
        let rung_colour = [
            near[0] + (far[0] - near[0]) * candidate as f64 / (LADDER_STEPS - 1) as f64,
            near[1] + (far[1] - near[1]) * candidate as f64 / (LADDER_STEPS - 1) as f64,
            near[2] + (far[2] - near[2]) * candidate as f64 / (LADDER_STEPS - 1) as f64,
        ];
        let measured = distance(&rung_colour, &source);
        if measured < closest {
            closest = measured;
            rung = candidate;
        }
    }
    let column =
        (y % DITHER_SIZE as u32) as usize * DITHER_SIZE + (x % DITHER_SIZE as u32) as usize;
    pack(if DITHER_MATRIX[rung][column] == 1 {
        &far
    } else {
        &near
    })
}

fn tied_triples(spec: FormatSpec) -> Vec<[i32; 3]> {
    let widths = [spec.red, spec.green, spec.blue];
    let mut blue_by_delta: HashMap<i32, Vec<i32>> = HashMap::new();
    for blue in 0..256 {
        blue_by_delta
            .entry(shade_delta(blue, widths[2]))
            .or_default()
            .push(blue);
    }
    let mut triples = Vec::new();
    for red in 0..256 {
        for green in 0..256 {
            let wanted = -(shade_delta(red, widths[0]) + shade_delta(green, widths[1]));
            if let Some(blues) = blue_by_delta.get(&wanted) {
                for blue in blues {
                    triples.push([red, green, *blue]);
                    if triples.len() >= TIE_TRIPLES {
                        return triples;
                    }
                }
            }
        }
    }
    triples
}

fn shade_delta(value: i32, bits: u32) -> i32 {
    ((f64::from(value) - lower_shade(value, bits)).abs()
        - (f64::from(value) - upper_shade(value, bits)).abs()) as i32
}

fn distance(first: &[f64; 3], second: &[f64; 3]) -> f64 {
    (first[0] - second[0]).abs() + (first[1] - second[1]).abs() + (first[2] - second[2]).abs()
}

fn pack(channels: &[f64; 3]) -> u32 {
    ((as_byte(channels[0]) as u32) << 16)
        | ((as_byte(channels[1]) as u32) << 8)
        | as_byte(channels[2]) as u32
}

fn as_byte(value: f64) -> i32 {
    if value <= 0.0 {
        0
    } else if value >= 255.0 {
        255
    } else {
        value.round_ties_even() as i32
    }
}

fn snap_alpha(value: i32, bits: u32) -> i32 {
    if bits >= 8 {
        return value;
    }
    let lower = lower_shade(value, bits);
    let upper = upper_shade(value, bits);
    if (f64::from(value) - lower).abs() > (f64::from(value) - upper).abs() {
        as_byte(upper)
    } else {
        as_byte(lower)
    }
}

fn lower_shade(value: i32, bits: u32) -> f64 {
    let scale = scale_for(bits);
    ((f64::from(value) / scale).floor() * scale + 0.5).floor()
}

fn upper_shade(value: i32, bits: u32) -> f64 {
    let scale = scale_for(bits);
    ((f64::from(value) / scale).ceil() * scale + 0.5).floor()
}

fn scale_for(bits: u32) -> f64 {
    255.0 / f64::from(255 >> (8 - bits))
}

fn assert_on_step(value: i32, code: u32, bits: u32, position: &str) {
    let scale = 255.0 / f64::from((1 << bits) - 1);
    let lower = ((f64::from(value) / scale).floor() * scale + 0.5).floor() as u32 >> (8 - bits);
    let upper = ((f64::from(value) / scale).ceil() * scale + 0.5).floor() as u32 >> (8 - bits);
    assert!(
        code == lower || code == upper,
        "{position} was {code}, expected {lower} or {upper} for {value}"
    );
}

fn expand(code: u32, shift: u32) -> i32 {
    if shift == 3 {
        ((code << 3) | (code >> 2)) as i32
    } else {
        ((code << 2) | (code >> 4)) as i32
    }
}

fn pixel(red: u8, green: u8, blue: u8, alpha: u8) -> Frame {
    Frame::new(1, 1, vec![red, green, blue, alpha])
}

fn encode_raster(raster: &Frame, format: VtfFormat, dither: bool) -> Vec<u8> {
    encode::encode(raster, format, Quality::Slow, dither, 2, None)
}

fn little_endian16(bytes: &[u8], offset: usize) -> u32 {
    u32::from(bytes[offset]) | (u32::from(bytes[offset + 1]) << 8)
}

fn gradient(width: u32, height: u32) -> Frame {
    let mut rgba = vec![0u8; (width * height * 4) as usize];
    for y in 0..height {
        for x in 0..width {
            let base = ((y * width + x) * 4) as usize;
            rgba[base] = (x * 8 + y) as u8;
            rgba[base + 1] = (y * 8 + x) as u8;
            rgba[base + 2] = (x * y) as u8;
            rgba[base + 3] = 0xff;
        }
    }
    Frame::new(width, height, rgba)
}

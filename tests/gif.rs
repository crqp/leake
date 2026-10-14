#![allow(
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]

mod common;

use common::options;
use leake::budget::FrameBudget;
use leake::format::VtfFormat;
use leake::frame::Frame;
use leake::gif;
use leake::options::SprayOptions;

const KEEP: u8 = 1;
const RESTORE_BACKGROUND: u8 = 2;
const RESTORE_PREVIOUS: u8 = 3;
const TRANSPARENT_INDEX: u8 = 0;

const RED: [u8; 4] = [255, 0, 0, 255];
const GREEN: [u8; 4] = [0, 255, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];

const RED_INDEX: u8 = 1;
const GREEN_INDEX: u8 = 2;
const BLUE_INDEX: u8 = 3;

const PALETTE: [u32; 4] = [0x00_0000, 0xff_0000, 0x00_ff00, 0x00_00ff];

#[test]
fn a_partial_frame_is_drawn_over_what_came_before_it() {
    let mut gif = GifBuilder::new(4, 4, &PALETTE);
    gif.frame(0, 0, 4, 4, &[RED_INDEX; 16], 20, KEEP, None);
    gif.frame(1, 1, 2, 2, &[GREEN_INDEX; 4], 20, KEEP, None);

    let frames = gif::read(&gif.build(), &all_frames_options(), None).expect("read");

    assert_eq!(2, frames.len());
    assert_eq!(RED, frames[0].pixel(3, 3));
    assert_eq!(GREEN, frames[1].pixel(1, 1));
    assert_eq!(RED, frames[1].pixel(3, 3));
}

#[test]
fn a_frame_disposed_to_the_background_leaves_nothing_behind() {
    let mut gif = GifBuilder::new(4, 4, &PALETTE);
    gif.frame(0, 0, 4, 4, &[RED_INDEX; 16], 20, RESTORE_BACKGROUND, None);
    gif.frame(0, 0, 2, 2, &[GREEN_INDEX; 4], 20, KEEP, None);

    let frames = gif::read(&gif.build(), &all_frames_options(), None).expect("read");

    assert_eq!(GREEN, frames[1].pixel(0, 0));
    assert_ne!(
        RED,
        frames[1].pixel(3, 3),
        "the first frame should have been cleared away"
    );
}

#[test]
fn a_frame_disposed_to_the_previous_one_restores_the_older_content() {
    let mut gif = GifBuilder::new(4, 4, &PALETTE);
    gif.frame(0, 0, 4, 4, &[RED_INDEX; 16], 20, KEEP, None);
    gif.frame(1, 1, 2, 2, &[GREEN_INDEX; 4], 20, RESTORE_PREVIOUS, None);
    gif.frame(2, 2, 2, 2, &[BLUE_INDEX; 4], 20, KEEP, None);

    let frames = gif::read(&gif.build(), &all_frames_options(), None).expect("read");

    assert_eq!(GREEN, frames[1].pixel(1, 1));
    assert_eq!(BLUE, frames[2].pixel(2, 2));
    assert_eq!(
        RED,
        frames[2].pixel(1, 1),
        "the green frame should have been rolled back"
    );
}

#[test]
fn transparent_pixels_keep_what_is_underneath() {
    let mut gif = GifBuilder::new(4, 4, &PALETTE);
    gif.frame(0, 0, 4, 4, &[RED_INDEX; 16], 20, KEEP, None);
    let pixels = [
        GREEN_INDEX,
        TRANSPARENT_INDEX,
        TRANSPARENT_INDEX,
        TRANSPARENT_INDEX,
    ];
    gif.frame(2, 2, 2, 2, &pixels, 20, KEEP, Some(TRANSPARENT_INDEX));

    let frames = gif::read(&gif.build(), &all_frames_options(), None).expect("read");

    assert_eq!(GREEN, frames[1].pixel(2, 2));
    assert_eq!(RED, frames[1].pixel(3, 3));
}

#[test]
fn frames_are_sampled_at_the_spray_playback_rate() {
    let mut gif = GifBuilder::new(2, 2, &PALETTE[..3]);
    gif.frame(0, 0, 2, 2, &[RED_INDEX; 4], 20, KEEP, None);
    gif.frame(0, 0, 2, 2, &[GREEN_INDEX; 4], 20, KEEP, None);

    let frames = gif::read(&gif.build(), &options_for(1.0, 0.0, false), None).expect("read");

    assert_eq!(2, frames.len());
    assert_eq!(RED, frames[0].pixel(0, 0));
    assert_eq!(GREEN, frames[1].pixel(0, 0));
}

#[test]
fn a_frame_shorter_than_a_slot_is_dropped_and_a_longer_one_repeats() {
    let mut quick = GifBuilder::new(2, 2, &PALETTE[..3]);
    quick.frame(0, 0, 2, 2, &[RED_INDEX; 4], 10, KEEP, None);
    quick.frame(0, 0, 2, 2, &[GREEN_INDEX; 4], 10, KEEP, None);
    assert_eq!(
        1,
        gif::read(&quick.build(), &options_for(1.0, 0.0, false), None)
            .expect("read")
            .len(),
        "both frames fit inside one slot"
    );

    let mut slow = GifBuilder::new(2, 2, &PALETTE[..3]);
    slow.frame(0, 0, 2, 2, &[RED_INDEX; 4], 10, KEEP, None);
    slow.frame(0, 0, 2, 2, &[GREEN_INDEX; 4], 50, KEEP, None);
    let slow_frames = gif::read(&slow.build(), &options_for(1.0, 0.0, false), None).expect("read");
    assert_eq!(3, slow_frames.len(), "the long frame covers three slots");
    assert_eq!(RED, slow_frames[0].pixel(0, 0));
    assert_eq!(GREEN, slow_frames[1].pixel(0, 0));
}

#[test]
fn the_speed_factor_stretches_the_slots() {
    let mut gif = GifBuilder::new(2, 2, &PALETTE[..3]);
    gif.frame(0, 0, 2, 2, &[RED_INDEX; 4], 10, KEEP, None);
    gif.frame(0, 0, 2, 2, &[GREEN_INDEX; 4], 50, KEEP, None);

    assert_eq!(
        2,
        gif::read(&gif.build(), &options_for(2.0, 0.0, false), None)
            .expect("read")
            .len(),
        "slots twice as long hold the same frames"
    );
}

#[test]
fn a_frame_with_no_delay_is_still_shown() {
    let mut gif = GifBuilder::new(2, 2, &PALETTE[..3]);
    gif.frame(0, 0, 2, 2, &[RED_INDEX; 4], 0, KEEP, None);
    gif.frame(0, 0, 2, 2, &[GREEN_INDEX; 4], 0, KEEP, None);

    let frames = gif::read(&gif.build(), &options_for(1.0, 0.0, false), None).expect("read");

    assert_eq!(
        1,
        frames.len(),
        "two frames of a tenth of a second fill one slot"
    );
    assert_eq!(RED, frames[0].pixel(0, 0));
}

#[test]
fn every_source_frame_can_be_imported_as_it_stands() {
    let mut gif = GifBuilder::new(2, 2, &PALETTE[..3]);
    gif.frame(0, 0, 2, 2, &[RED_INDEX; 4], 10, KEEP, None);
    gif.frame(0, 0, 2, 2, &[GREEN_INDEX; 4], 10, KEEP, None);

    let frames = gif::read(&gif.build(), &all_frames_options(), None).expect("read");

    assert_eq!(2, frames.len());
}

#[test]
fn a_frame_range_selects_which_frames_are_imported() {
    let mut gif = GifBuilder::new(2, 2, &PALETTE[..3]);
    gif.frame(0, 0, 2, 2, &[RED_INDEX; 4], 20, KEEP, None);
    gif.frame(0, 0, 2, 2, &[GREEN_INDEX; 4], 20, KEEP, None);

    let frames = gif::read(&gif.build(), &options_for(1.0, 1.0, false), None).expect("read");

    assert_eq!(1, frames.len());
    assert_eq!(GREEN, frames[0].pixel(0, 0));
}

#[test]
fn importing_stops_once_the_spray_size_limit_is_reached() {
    let mut gif = GifBuilder::new(2, 2, &PALETTE[..3]);
    gif.frame(0, 0, 2, 2, &[RED_INDEX; 4], 20, KEEP, None);
    gif.frame(0, 0, 2, 2, &[GREEN_INDEX; 4], 20, KEEP, None);

    let mut budget = FrameBudget::new(1024, 1024, VtfFormat::Dxt1);
    let frames = gif::read(
        &gif.build(),
        &options_for(1.0, 0.0, false),
        Some(&mut budget),
    )
    .expect("read");

    assert_eq!(1, frames.len());
    assert!(budget.truncated);
}

#[test]
fn a_still_is_not_treated_as_an_animation() {
    let mut gif = GifBuilder::new(2, 2, &PALETTE[..2]);
    gif.frame(0, 0, 2, 2, &[RED_INDEX; 4], 20, KEEP, None);

    assert!(!gif::is_animated(&gif.build()));
    assert_eq!(RED, as_frame(&gif).pixel(0, 0));
}

#[test]
fn a_still_gif_can_be_imported_with_all_its_transparency_intact() {
    let mut gif = GifBuilder::new(2, 2, &PALETTE[..2]);
    gif.frame(0, 0, 2, 2, &[RED_INDEX; 4], 20, KEEP, Some(0));
    assert_eq!(RED, as_frame(&gif).pixel(0, 0));
}

fn as_frame(builder: &GifBuilder) -> Frame {
    let frames = gif::read(&builder.build(), &all_frames_options(), None).expect("read");
    assert_eq!(1, frames.len());
    frames.into_iter().next().expect("one frame")
}

fn all_frames_options() -> SprayOptions {
    options_for(1.0, 0.0, true)
}

fn options_for(speed: f64, start: f64, all_frames: bool) -> SprayOptions {
    SprayOptions {
        speed,
        start,
        all_frames,
        ..options(Vec::new())
    }
}

struct GifBuilder {
    output: Vec<u8>,
    table_bits: u32,
}

impl GifBuilder {
    fn new(width: u16, height: u16, palette: &[u32]) -> Self {
        let mut table_bits = 1;
        while (1 << table_bits) < palette.len() {
            table_bits += 1;
        }
        let entries = 1usize << table_bits;
        let mut output = Vec::new();
        output.extend_from_slice(b"GIF89a");
        write_short(&mut output, width);
        write_short(&mut output, height);
        output.push(GLOBAL_COLOUR_TABLE | ((table_bits as u8 - 1) << 4) | (table_bits as u8 - 1));
        output.push(0);
        output.push(0);
        for index in 0..entries {
            let colour = palette.get(index).copied().unwrap_or(0);
            output.push(((colour >> 16) & 0xff) as u8);
            output.push(((colour >> 8) & 0xff) as u8);
            output.push((colour & 0xff) as u8);
        }
        Self { output, table_bits }
    }

    #[allow(clippy::too_many_arguments)]
    fn frame(
        &mut self,
        left: u16,
        top: u16,
        width: u16,
        height: u16,
        pixels: &[u8],
        delay: u16,
        disposal: u8,
        transparent_index: Option<u8>,
    ) {
        self.output.push(EXTENSION);
        self.output.push(GRAPHIC_CONTROL);
        self.output.push(4);
        let mut packed = disposal << 2;
        let mut transparent = 0;
        if let Some(index) = transparent_index {
            packed |= 1;
            transparent = index;
        }
        self.output.push(packed);
        write_short(&mut self.output, delay);
        self.output.push(transparent);
        self.output.push(0);

        self.output.push(IMAGE_DESCRIPTOR);
        write_short(&mut self.output, left);
        write_short(&mut self.output, top);
        write_short(&mut self.output, width);
        write_short(&mut self.output, height);
        self.output.push(0);

        let code_size = if self.table_bits < 2 {
            2
        } else {
            self.table_bits
        };
        let clear_code = 1u32 << code_size;
        let end_code = clear_code + 1;
        let code_width = code_size + 1;
        self.output.push(code_size as u8);
        let mut codes = BitWriter::new();
        for pixel in pixels {
            codes.write(clear_code, code_width);
            codes.write(u32::from(*pixel), code_width);
        }
        codes.write(end_code, code_width);

        let data = codes.finish();
        let mut offset = 0;
        while offset < data.len() {
            let size = 255.min(data.len() - offset);
            self.output.push(size as u8);
            self.output.extend_from_slice(&data[offset..offset + size]);
            offset += size;
        }
        self.output.push(0);
    }

    fn build(&self) -> Vec<u8> {
        let mut output = self.output.clone();
        output.push(TRAILER);
        output
    }
}

const EXTENSION: u8 = 0x21;
const GRAPHIC_CONTROL: u8 = 0xF9;
const IMAGE_DESCRIPTOR: u8 = 0x2C;
const TRAILER: u8 = 0x3B;
const GLOBAL_COLOUR_TABLE: u8 = 0x80;

fn write_short(output: &mut Vec<u8>, value: u16) {
    output.push((value & 0xff) as u8);
    output.push((value >> 8) as u8);
}

struct BitWriter {
    output: Vec<u8>,
    current: u8,
    bits: u32,
}

impl BitWriter {
    fn new() -> Self {
        Self {
            output: Vec::new(),
            current: 0,
            bits: 0,
        }
    }

    fn write(&mut self, value: u32, width: u32) {
        for bit in 0..width {
            if (value >> bit) & 1 == 1 {
                self.current |= 1 << self.bits;
            }
            self.bits += 1;
            if self.bits == 8 {
                self.output.push(self.current);
                self.current = 0;
                self.bits = 0;
            }
        }
    }

    fn finish(mut self) -> Vec<u8> {
        if self.bits > 0 {
            self.output.push(self.current);
        }
        self.output
    }
}

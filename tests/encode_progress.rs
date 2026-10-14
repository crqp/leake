mod common;

use common::noisy_frame;
use leake::encode;
use leake::format::{Quality, VtfFormat};

#[test]
fn dxt_reports_block_rows() {
    let pairs = encode_with_sink(VtfFormat::Dxt1, 32, 16, false);
    assert_eq!(
        4,
        pairs.last().expect("reports").1,
        "a 16 pixel tall raster is four block rows tall"
    );
    assert_monotonic(&pairs);
}

#[test]
fn dxt_pads_partial_block_rows_into_the_total() {
    let pairs = encode_with_sink(VtfFormat::Dxt5, 17, 10, false);
    assert_eq!(
        3,
        pairs.last().expect("reports").1,
        "10 pixels tall pads up to three block rows"
    );
    assert_monotonic(&pairs);
}

#[test]
fn packed_formats_report_pixel_rows() {
    let pairs = encode_with_sink(VtfFormat::Rgba8888, 24, 7, false);
    assert_eq!(
        7,
        pairs.last().expect("reports").1,
        "a packed format counts every pixel row"
    );
    assert_monotonic(&pairs);
}

#[test]
fn packed_dithering_reports_pixel_rows_too() {
    let pairs = encode_with_sink(VtfFormat::Rgb565, 24, 7, true);
    assert_eq!(7, pairs.last().expect("reports").1);
    assert_monotonic(&pairs);
}

fn encode_with_sink(
    format: VtfFormat,
    width: u32,
    height: u32,
    dither: bool,
) -> Vec<(usize, usize)> {
    let mut pairs = Vec::new();
    {
        let mut sink = |done: usize, total: usize| pairs.push((done, total));
        let _ = encode::encode(
            &noisy_frame(width, height),
            format,
            Quality::Fast,
            dither,
            1,
            Some(&mut sink),
        );
    }
    assert!(!pairs.is_empty(), "the sink was never called");
    pairs
}

fn assert_monotonic(pairs: &[(usize, usize)]) {
    let done: Vec<usize> = pairs.iter().map(|(done, _)| *done).collect();
    let mut sorted = done.clone();
    sorted.sort_unstable();
    assert_eq!(sorted, done, "reported counts went backwards");
    let total = pairs.last().expect("reports").1;
    assert!(
        pairs.iter().all(|(_, pair_total)| *pair_total == total),
        "the reported total moved mid encode"
    );
    assert_eq!(
        total,
        *done.last().expect("reports"),
        "the sink never reported the encode finishing"
    );
}

#![allow(
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]

mod common;

use common::noisy_frame;
use leake::encode;
use leake::format::{Quality, VtfFormat};
use leake::frame::Frame;

#[test]
#[ignore = "throughput ceilings are measured on optimised builds; run with cargo test --release -- --ignored"]
fn per_format_throughput_stays_within_ceiling() {
    let jobs = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);
    for case in cases() {
        let raster = noisy_frame(case.size, case.size);
        for _ in 0..case.warmups {
            encode_raster(&raster, &case, jobs);
        }
        let started = std::time::Instant::now();
        encode_raster(&raster, &case, jobs);
        let nanos = started.elapsed().as_nanos();
        let blocks = (case.size / 4) * (case.size / 4);
        println!(
            "throughput {:<8} {:<9} {:3}x{:<3} {:8.2} ms {:9.0} ns/block",
            case.format,
            case.quality,
            case.size,
            case.size,
            nanos as f64 / 1_000_000.0,
            nanos as f64 / f64::from(blocks),
        );
        assert!(
            nanos <= u128::from(case.millis_allowed) * 1_000_000,
            "{} at {} on {}x{} took {:.1} ms",
            case.format,
            case.quality,
            case.size,
            case.size,
            nanos as f64 / 1_000_000.0
        );
    }
}

fn encode_raster(raster: &Frame, case: &ThroughputCase, jobs: usize) {
    let _ = encode::encode(raster, case.format, case.quality, true, jobs, None);
}

struct ThroughputCase {
    format: VtfFormat,
    quality: Quality,
    size: u32,
    millis_allowed: u64,
    warmups: usize,
}

fn case(format: VtfFormat, quality: Quality, size: u32, millis_allowed: u64) -> ThroughputCase {
    ThroughputCase {
        format,
        quality,
        size,
        millis_allowed,
        warmups: 3,
    }
}

fn case_with_warmups(
    format: VtfFormat,
    quality: Quality,
    size: u32,
    millis_allowed: u64,
    warmups: usize,
) -> ThroughputCase {
    ThroughputCase {
        format,
        quality,
        size,
        millis_allowed,
        warmups,
    }
}

fn cases() -> Vec<ThroughputCase> {
    vec![
        case(VtfFormat::Dxt1, Quality::Fast, 64, 500),
        case(VtfFormat::Dxt1, Quality::Normal, 64, 500),
        case(VtfFormat::Dxt1, Quality::Slow, 64, 1_500),
        case(VtfFormat::Dxt1, Quality::VerySlow, 64, 8_000),
        case(VtfFormat::Dxt1, Quality::Fast, 256, 500),
        case(VtfFormat::Dxt1, Quality::Normal, 256, 1_000),
        case(VtfFormat::Dxt1, Quality::Slow, 256, 1_500),
        case(VtfFormat::Dxt5, Quality::Fast, 64, 500),
        case(VtfFormat::Dxt5, Quality::Normal, 64, 500),
        case(VtfFormat::Dxt5, Quality::Slow, 64, 500),
        case(VtfFormat::Dxt5, Quality::VerySlow, 64, 4_000),
        case(VtfFormat::Dxt5, Quality::Fast, 256, 500),
        case(VtfFormat::Dxt5, Quality::Normal, 256, 500),
        case(VtfFormat::Dxt5, Quality::Slow, 256, 1_000),
        case(VtfFormat::Rgb565, Quality::Fast, 64, 500),
        case(VtfFormat::Rgb565, Quality::Normal, 64, 500),
        case(VtfFormat::Rgb565, Quality::Slow, 64, 500),
        case(VtfFormat::Rgb565, Quality::VerySlow, 64, 500),
        case(VtfFormat::Rgb565, Quality::Fast, 256, 500),
        case(VtfFormat::Rgb565, Quality::Normal, 256, 500),
        case(VtfFormat::Rgb565, Quality::Slow, 256, 500),
        case_with_warmups(VtfFormat::Dxt1, Quality::VerySlow, 128, 21_000, 1),
        case_with_warmups(VtfFormat::Dxt5, Quality::VerySlow, 128, 12_000, 1),
        case(VtfFormat::Rgb565, Quality::VerySlow, 128, 500),
    ]
}

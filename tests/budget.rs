mod common;

use leake::budget::{self, FrameBudget, LayoutRequest, MAX_ATLAS_HEIGHT};
use leake::format::{Sampling, VtfFormat};

#[test]
fn byte_estimates_add_the_header_after_the_mipmap_payload() {
    assert_eq!(2792.0, budget::est_bytes(64, 64, 1, VtfFormat::Dxt1, true));
}

#[test]
fn resolution_falls_as_the_frame_count_rises() {
    let cases: [(u32, u32); 11] = [
        (1, 1024),
        (2, 512),
        (3, 512),
        (4, 512),
        (5, 256),
        (17, 128),
        (65, 64),
        (257, 32),
        (1025, 16),
        (4097, 8),
        (16_385, 4),
    ];
    for (frames, expected) in cases {
        assert_eq!(
            expected,
            budget::clamp_resolution(1024, 1024, frames),
            "{frames} frames"
        );
    }
}

#[test]
fn resolution_is_capped_by_the_source_size() {
    assert_eq!(64, budget::clamp_resolution(60, 40, 1));
    assert_eq!(256, budget::clamp_resolution(200, 255, 1));
    assert_eq!(
        4,
        budget::clamp_resolution(3, 3, 1),
        "never below the minimum"
    );
}

#[test]
fn a_single_large_frame_loses_four_pixels_of_width_to_fit_the_ceiling() {
    let layout = budget::resolve(&LayoutRequest::new(1024, 1024, 1)).expect("resolve");
    assert!(
        layout.shortened,
        "1024x1024 DXT1 lands just above the ceiling"
    );
    assert_eq!(1020, layout.width);
    assert_eq!(1024, layout.height);
}

#[test]
fn a_texture_that_comfortably_fits_is_left_alone() {
    let layout = budget::resolve(&LayoutRequest::new(1024, 1024, 2)).expect("resolve");
    assert!(!layout.shortened);
    assert_eq!(512, layout.width);
    assert_eq!(512, layout.height);
}

#[test]
fn mipmaps_need_whole_blocks_and_a_texture_below_the_ceiling() {
    assert!(allowed(512, 512, 1, VtfFormat::Dxt1, Sampling::Default));
    assert!(
        !allowed(512, 512, 1, VtfFormat::Dxt1, Sampling::Point),
        "point sampling has no use for mipmaps"
    );
    assert!(
        !allowed(500, 512, 1, VtfFormat::Dxt1, Sampling::Default),
        "width is not a whole number of blocks"
    );
    assert!(
        !allowed(512, 500, 1, VtfFormat::Dxt1, Sampling::Default),
        "height is not a whole number of blocks"
    );
    assert!(
        !allowed(512, 512, 16, VtfFormat::Dxt5, Sampling::Default),
        "too large for mipmaps to help"
    );
}

#[test]
fn mipmap_levels_halve_until_they_would_be_too_small_to_sample() {
    assert_eq!(5, budget::mip_count(1024, 1024));
    assert_eq!(4, budget::mip_count(512, 512));
    assert_eq!(1, budget::mip_count(64, 64));
    assert_eq!(0, budget::mip_count(32, 32));
    assert_eq!(0, budget::mip_count(32, 1024));
}

#[test]
fn the_mipmap_count_drives_the_number_of_levels_written() {
    let layout = budget::resolve(&LayoutRequest {
        mipmaps_requested: Some(true),
        ..LayoutRequest::new(512, 512, 1)
    })
    .expect("resolve");
    assert!(layout.mipmaps);
    assert_eq!(4, layout.mip_count);
}

#[test]
fn mipmaps_can_be_forced_on_and_off() {
    let off = budget::resolve(&LayoutRequest {
        mipmaps_requested: Some(false),
        ..LayoutRequest::new(512, 512, 1)
    })
    .expect("resolve");
    assert!(!off.mipmaps);
    assert_eq!(0, off.mip_count);

    let forced = budget::resolve(&LayoutRequest {
        requested_width: Some(500),
        requested_height: Some(500),
        mipmaps_requested: Some(true),
        ..LayoutRequest::new(500, 500, 1)
    })
    .expect("resolve");
    assert!(forced.mipmaps, "the caller can override the gate");
    assert!(!forced.mipmaps_permitted);
}

#[test]
fn an_explicit_size_is_used_as_given() {
    let layout = budget::resolve(&LayoutRequest {
        requested_width: Some(256),
        requested_height: Some(128),
        ..LayoutRequest::new(1024, 1024, 1)
    })
    .expect("resolve");
    assert_eq!(256, layout.width);
    assert_eq!(128, layout.height);
}

#[test]
fn one_side_on_its_own_sets_both() {
    let layout = budget::resolve(&LayoutRequest {
        format: VtfFormat::Rgb565,
        requested_width: Some(128),
        ..LayoutRequest::new(1024, 1024, 1)
    })
    .expect("resolve");
    assert_eq!(128, layout.width);
    assert_eq!(128, layout.height);
}

#[test]
fn a_texture_too_large_for_a_spray_is_refused() {
    let failure = budget::resolve(&LayoutRequest {
        format: VtfFormat::Rgba8888,
        requested_width: Some(512),
        requested_height: Some(512),
        ..LayoutRequest::new(1024, 1024, 62)
    })
    .expect_err("too large");
    assert!(failure.to_string().contains("too large"), "{failure}");
}

#[test]
fn a_frame_count_that_cannot_fit_the_height_limit_is_refused() {
    let failure = budget::resolve(&LayoutRequest {
        requested_width: Some(1024),
        requested_height: Some(1024),
        ..LayoutRequest::new(1024, 1024, 400)
    })
    .expect_err("tall texture");
    assert!(failure.to_string().contains("tall texture"), "{failure}");
}

#[test]
fn an_automatic_size_keeps_frames_within_the_height_the_engine_can_address() {
    let layout = budget::resolve(&LayoutRequest::new(1024, 1024, 2000)).expect("resolve");
    assert!(
        layout.height * layout.frames <= MAX_ATLAS_HEIGHT,
        "atlas is {}px tall",
        layout.height * layout.frames
    );
}

#[test]
fn the_frame_budget_stops_importing_when_the_spray_would_grow_too_large() {
    let mut frame_budget = FrameBudget::new(1024, 1024, VtfFormat::Dxt1);
    assert!(!frame_budget.stop(), "one frame fits");
    assert!(frame_budget.stop(), "two frames do not");
    assert!(frame_budget.truncated);
}

fn allowed(width: u32, height: u32, frames: u32, format: VtfFormat, sampling: Sampling) -> bool {
    budget::mipmaps_allowed(width, height, frames, format, sampling, false)
}

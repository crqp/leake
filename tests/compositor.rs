mod common;

use std::collections::HashMap;

use leake::budget::Layout;
use leake::compositor;
use leake::frame::Frame;

fn layout() -> Layout {
    Layout {
        width: 8,
        height: 4,
        frames: 2,
        shortened: false,
        mipmaps: false,
        mipmaps_permitted: false,
        mip_count: 0,
    }
}

#[test]
fn frames_are_stacked_vertically() {
    let frames = vec![solid(2, 2, 255, 0, 0), solid(2, 2, 0, 255, 0)];
    let atlas = compositor::build(&frames, &layout(), false, &HashMap::new())
        .into_iter()
        .next()
        .expect("atlas");

    assert_eq!(8, atlas.width);
    assert_eq!(8, atlas.height);
    assert_eq!(255, atlas.pixel(3, 1)[0]);
    assert_eq!(0, atlas.pixel(3, 5)[0]);
    assert_eq!(255, atlas.pixel(3, 5)[1]);
}

#[test]
fn an_unscaled_frame_is_centred_with_clear_margins() {
    let frames = vec![solid(2, 2, 255, 0, 0), solid(2, 2, 255, 0, 0)];
    let atlas = compositor::build(&frames, &layout(), false, &HashMap::new())
        .into_iter()
        .next()
        .expect("atlas");

    assert_eq!(0, atlas.pixel(0, 0)[3], "the margin should be clear");
    assert_eq!(0, atlas.pixel(7, 3)[3], "the margin should be clear");
    assert_eq!(
        255,
        atlas.pixel(3, 1)[3],
        "the frame itself should be opaque"
    );
}

#[test]
fn an_oversized_frame_is_cropped_rather_than_shrunk_when_rescaling_is_off() {
    let frames = vec![solid(16, 8, 255, 0, 0), solid(16, 8, 255, 0, 0)];
    let atlas = compositor::build(&frames, &layout(), false, &HashMap::new())
        .into_iter()
        .next()
        .expect("atlas");

    assert_eq!(
        255,
        atlas.pixel(0, 0)[3],
        "the cropped middle of the frame should still be filled"
    );
    assert_eq!(255, atlas.pixel(7, 7)[3]);
    assert_eq!(255, atlas.pixel(0, 7)[0]);
}

#[test]
fn a_frame_is_scaled_to_fit_its_slot() {
    let frames = vec![solid(16, 16, 255, 0, 0), solid(16, 16, 255, 0, 0)];
    let atlas = compositor::build(&frames, &layout(), true, &HashMap::new())
        .into_iter()
        .next()
        .expect("atlas");

    assert_eq!(
        255,
        atlas.pixel(2, 0)[3],
        "scaling to fit should reach the top of the slot"
    );
    assert_eq!(
        255,
        atlas.pixel(5, 3)[3],
        "scaling to fit should reach the bottom of the slot"
    );
    assert_eq!(
        0,
        atlas.pixel(0, 0)[3],
        "the remaining width should be left clear"
    );
    assert_eq!(
        0,
        atlas.pixel(7, 3)[3],
        "the remaining width should be left clear"
    );
}

#[test]
fn mipmap_levels_halve_and_keep_every_frame() {
    let frames = vec![solid(8, 8, 255, 0, 0), solid(8, 8, 0, 0, 255)];
    let big = Layout {
        width: 64,
        height: 64,
        mipmaps: true,
        mipmaps_permitted: true,
        mip_count: 2,
        ..layout()
    };
    let levels = compositor::build(&frames, &big, true, &HashMap::new());

    assert_eq!(3, levels.len());
    assert_eq!((64, 128), (levels[0].width, levels[0].height));
    assert_eq!((32, 64), (levels[1].width, levels[1].height));
    assert_eq!((16, 32), (levels[2].width, levels[2].height));
    assert!(
        levels[1].pixel(16, 16)[0] > 0,
        "the second level should still show the first frame"
    );
}

#[test]
fn replacement_images_are_cycled_to_cover_every_frame() {
    let frames = vec![
        solid(4, 4, 255, 0, 0),
        solid(4, 4, 255, 0, 0),
        solid(4, 4, 255, 0, 0),
    ];
    let big = Layout {
        width: 32,
        height: 32,
        frames: 3,
        mipmaps: true,
        mipmaps_permitted: true,
        mip_count: 1,
        ..layout()
    };
    let mut custom = HashMap::new();
    custom.insert(1, vec![solid(4, 4, 0, 255, 0)]);
    let levels = compositor::build(&frames, &big, true, &custom);

    assert_eq!(
        0,
        levels[1].pixel(0, 0)[0],
        "the replacement image should have been used"
    );
    assert_eq!(255, levels[1].pixel(0, 0)[1]);
}

#[test]
fn a_strip_is_cut_into_frames_of_the_requested_height() {
    let strip = Frame::new(2, 8, vec![0xff; 2 * 8 * 4]);
    let frames = compositor::slice_strip(&strip, 4).expect("slice");

    assert_eq!(2, frames.len());
    assert_eq!(4, frames[0].height);
    assert_eq!(2, frames[0].width);
}

#[test]
fn a_strip_shorter_than_one_frame_is_refused() {
    let strip = Frame::new(2, 3, vec![0; 2 * 3 * 4]);
    let failure = compositor::slice_strip(&strip, 4).expect_err("less than one frame");
    assert!(
        failure.to_string().contains("less than one frame"),
        "{failure}"
    );
}

fn solid(width: u32, height: u32, red: u8, green: u8, blue: u8) -> Frame {
    let mut rgba = vec![0u8; (width * height * 4) as usize];
    for pixel in 0..(width * height) as usize {
        rgba[pixel * 4] = red;
        rgba[pixel * 4 + 1] = green;
        rgba[pixel * 4 + 2] = blue;
        rgba[pixel * 4 + 3] = 0xff;
    }
    Frame::new(width, height, rgba)
}

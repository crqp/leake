mod common;

use common::{fixture, options};
use leake::frame::Frame;
use leake::input::{self, DecodedInput};

const RED: [u8; 4] = [255, 0, 0, 255];
const GREEN: [u8; 4] = [0, 255, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];
const YELLOW: [u8; 4] = [255, 255, 0, 255];

#[test]
fn decodes_a_png_exactly() {
    assert_quadrants(&single("sample.png"));
}

#[test]
fn decodes_lossless_formats_exactly() {
    for name in ["sample.bmp", "sample.tga", "sample.tiff", "sample.webp"] {
        assert_quadrants(&single(name));
    }
}

#[test]
fn decodes_jpeg_within_its_compression_tolerance() {
    let frame = single("sample.jpg");
    assert_eq!(16, frame.width);
    assert_close(frame.pixel(3, 3), RED, 24);
    assert_close(frame.pixel(12, 3), GREEN, 24);
    assert_close(frame.pixel(3, 12), BLUE, 24);
    assert_close(frame.pixel(12, 12), YELLOW, 24);
}

#[test]
fn keeps_alpha_from_a_png() {
    let frame = single("alpha.png");
    assert_eq!([255, 0, 0, 128], frame.pixel(3, 3));
    assert_eq!([0, 0, 255, 64], frame.pixel(12, 3));
}

#[test]
fn several_images_become_animation_frames() {
    let inputs = vec![fixture("sample.png"), fixture("sample.bmp")];
    let decoded = input::read(&options(inputs), None).expect("read");
    let DecodedInput::Frames(frames) = decoded else {
        panic!("expected frames");
    };
    assert_eq!(2, frames.len());
}

#[test]
fn reports_the_input_it_cannot_read() {
    let directory = tempfile::tempdir().expect("temp dir");
    let unreadable = directory.path().join("not-an-image.dat");
    std::fs::write(&unreadable, "this is not an image").expect("write");
    let Err(failure) = input::read(&options(vec![unreadable]), None) else {
        panic!("expected the unreadable file to fail");
    };
    assert!(
        failure.to_string().contains("not-an-image.dat"),
        "{failure}"
    );
}

fn single(name: &str) -> Frame {
    let decoded = input::read(&options(vec![fixture(name)]), None).expect("read");
    let DecodedInput::Frames(frames) = decoded else {
        panic!("expected frames");
    };
    assert_eq!(1, frames.len());
    frames.into_iter().next().expect("one frame")
}

fn assert_quadrants(frame: &Frame) {
    assert_eq!(16, frame.width);
    assert_eq!(16, frame.height);
    assert_eq!(RED, frame.pixel(3, 3));
    assert_eq!(GREEN, frame.pixel(12, 3));
    assert_eq!(BLUE, frame.pixel(3, 12));
    assert_eq!(YELLOW, frame.pixel(12, 12));
}

fn assert_close(actual: [u8; 4], expected: [u8; 4], tolerance: i32) {
    for channel in 0..4 {
        let difference = i32::from(actual[channel]) - i32::from(expected[channel]);
        assert!(
            difference.abs() <= tolerance,
            "channel {channel} was {}, expected about {}",
            actual[channel],
            expected[channel]
        );
    }
}

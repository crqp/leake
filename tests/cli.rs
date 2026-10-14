mod common;

use std::path::Path;
use std::process::Command;

use common::block_decoder::decode_dxt1;
use common::fixture;
use leake::format::VtfFormat;

#[test]
fn writes_a_texture_and_a_material_file() {
    let directory = tempfile::tempdir().expect("temp dir");
    let source = fixture("sample.png");
    let (code, stdout, stderr) = run(&[
        "-d",
        &directory_arg(directory.path()),
        "-w",
        "32",
        "-q",
        "fast",
        &source_arg(&source),
    ]);

    assert_eq!(0, code, "{stdout}{stderr}");
    let vtf = directory.path().join("spray.vtf");
    let vmt = directory.path().join("spray.vmt");
    assert!(vtf.exists());
    assert!(vmt.exists());

    let bytes = std::fs::read(&vtf).expect("read vtf");
    assert_eq!("VTF", signature(&bytes));
    assert_eq!(32, u16_at(&bytes, 16));
    assert_eq!(32, u16_at(&bytes, 18));
    assert_eq!(1, u16_at(&bytes, 24));
    assert_eq!(VtfFormat::Dxt1.id(), u32_at(&bytes, 52));
    assert_eq!(
        64 + 8 * (32 / 4) * (32 / 4),
        bytes.len(),
        "a DXT1 level is eight bytes a block"
    );

    let material = std::fs::read_to_string(&vmt).expect("read vmt");
    assert!(
        material.contains("\"$basetexture\"\t\"vgui/logos/spray\""),
        "{material}"
    );
}

#[test]
fn the_material_file_can_be_left_out() {
    let directory = tempfile::tempdir().expect("temp dir");
    let source = fixture("sample.png");
    let (code, stdout, stderr) = run(&[
        "-d",
        &directory_arg(directory.path()),
        "-w",
        "16",
        "--no-vmt",
        &source_arg(&source),
    ]);

    assert_eq!(0, code, "{stdout}{stderr}");
    assert!(directory.path().join("spray.vtf").exists());
    assert!(!directory.path().join("spray.vmt").exists());
}

#[test]
fn the_output_name_is_used_for_both_files() {
    let directory = tempfile::tempdir().expect("temp dir");
    let source = fixture("sample.png");
    let (code, stdout, stderr) = run(&[
        "-d",
        &directory_arg(directory.path()),
        "-w",
        "16",
        "-o",
        "logo",
        &source_arg(&source),
    ]);

    assert_eq!(0, code, "{stdout}{stderr}");
    assert!(directory.path().join("logo.vtf").exists());
    let material = std::fs::read_to_string(directory.path().join("logo.vmt")).expect("read vmt");
    assert!(material.contains("vgui/logos/logo"));
}

#[test]
fn a_dry_run_reports_the_layout_without_writing_anything() {
    let directory = tempfile::tempdir().expect("temp dir");
    let source = fixture("sample.png");
    let (code, stdout, stderr) = run(&[
        "-d",
        &directory_arg(directory.path()),
        "-w",
        "64",
        "--dry-run",
        &source_arg(&source),
    ]);

    assert_eq!(0, code, "{stdout}{stderr}");
    assert!(stdout.contains("size        64x64"), "{stdout}");
    assert!(stdout.contains("frames      1"), "{stdout}");
    assert!(
        stdout.contains("mip levels  2"),
        "64 pixels halves once before the level gets too small\n{stdout}"
    );
    assert!(stdout.contains("estimated   2.7 KiB"), "{stdout}");
    assert!(!directory.path().join("spray.vtf").exists());
}

#[test]
fn several_images_become_an_animation() {
    let directory = tempfile::tempdir().expect("temp dir");
    let first = fixture("sample.png");
    let second = fixture("sample.bmp");
    let (code, stdout, stderr) = run(&[
        "-d",
        &directory_arg(directory.path()),
        "-w",
        "32",
        "-q",
        "fast",
        &source_arg(&first),
        &source_arg(&second),
    ]);

    assert_eq!(0, code, "{stdout}{stderr}");
    let bytes = std::fs::read(directory.path().join("spray.vtf")).expect("read vtf");
    assert_eq!(2, u16_at(&bytes, 24));
}

#[test]
fn a_tall_image_is_cut_into_frames() {
    let directory = tempfile::tempdir().expect("temp dir");
    let source = fixture("strip.png");
    let (code, stdout, stderr) = run(&[
        "-d",
        &directory_arg(directory.path()),
        "--strip",
        "-w",
        "16",
        "--no-mipmaps",
        &source_arg(&source),
    ]);

    assert_eq!(0, code, "{stdout}{stderr}");
    let bytes = std::fs::read(directory.path().join("spray.vtf")).expect("read vtf");
    assert_eq!(
        4,
        u16_at(&bytes, 24),
        "the strip holds four frames of sixteen pixels"
    );
    assert_eq!(16, u16_at(&bytes, 18));
}

#[test]
fn an_animated_gif_is_sampled_at_the_spray_playback_rate() {
    let directory = tempfile::tempdir().expect("temp dir");
    let source = fixture("animated.gif");
    let (code, stdout, stderr) = run(&[
        "-d",
        &directory_arg(directory.path()),
        "-w",
        "16",
        &source_arg(&source),
    ]);

    assert_eq!(0, code, "{stdout}{stderr}");
    let bytes = std::fs::read(directory.path().join("spray.vtf")).expect("read vtf");
    assert_eq!(
        2,
        u16_at(&bytes, 24),
        "four frames of a tenth of a second fill two slots"
    );
}

#[test]
fn an_animated_gif_can_keep_every_frame() {
    let directory = tempfile::tempdir().expect("temp dir");
    let source = fixture("animated.gif");
    let (code, stdout, stderr) = run(&[
        "-d",
        &directory_arg(directory.path()),
        "-w",
        "16",
        "--all-frames",
        &source_arg(&source),
    ]);

    assert_eq!(0, code, "{stdout}{stderr}");
    let bytes = std::fs::read(directory.path().join("spray.vtf")).expect("read vtf");
    assert_eq!(4, u16_at(&bytes, 24));
}

#[test]
fn mipmap_levels_are_written_for_a_size_that_supports_them() {
    let directory = tempfile::tempdir().expect("temp dir");
    let source = fixture("sample.png");
    let (code, stdout, stderr) = run(&[
        "-d",
        &directory_arg(directory.path()),
        "-w",
        "128",
        "-q",
        "fast",
        &source_arg(&source),
    ]);

    assert_eq!(0, code, "{stdout}{stderr}");
    let bytes = std::fs::read(directory.path().join("spray.vtf")).expect("read vtf");
    assert_eq!(3, bytes[56], "128 halves to 64 and 32");
}

#[test]
fn negated_flags_take_effect() {
    let directory = tempfile::tempdir().expect("temp dir");
    let source = fixture("sample.png");
    let (code, stdout, stderr) = run(&[
        "-d",
        &directory_arg(directory.path()),
        "-w",
        "64",
        "--no-vmt",
        "--no-mipmaps",
        "--no-dither",
        "--no-rescale",
        "-q",
        "fast",
        &source_arg(&source),
    ]);

    assert_eq!(0, code, "{stdout}{stderr}");
    assert!(
        !directory.path().join("spray.vmt").exists(),
        "--no-vmt should suppress the material file"
    );
    let bytes = std::fs::read(directory.path().join("spray.vtf")).expect("read vtf");
    assert_eq!(
        1, bytes[56],
        "--no-mipmaps should suppress every level above the base"
    );
}

#[test]
fn transparency_survives_every_quality_setting() {
    for quality in ["fast", "normal", "slow", "very_slow"] {
        let directory = tempfile::tempdir().expect("temp dir");
        let source = fixture("alpha.png");
        let (code, stdout, stderr) = run(&[
            "-d",
            &directory_arg(directory.path()),
            "-w",
            "16",
            "--no-mipmaps",
            "-q",
            quality,
            &source_arg(&source),
        ]);

        assert_eq!(0, code, "{stdout}{stderr}");
        let bytes = std::fs::read(directory.path().join("spray.vtf")).expect("read vtf");
        let decoded = decode_dxt1(&bytes, 64, 16 / 4, 16, 16);
        let clear = decoded.iter().filter(|pixel| *pixel >> 24 == 0).count();
        assert_eq!(
            128, clear,
            "the half of the texture that is clear should stay clear at -q {quality}"
        );
    }
}

#[test]
fn a_replacement_image_is_used_for_the_level_it_names() {
    let directory = tempfile::tempdir().expect("temp dir");
    let source = fixture("sample.png");
    let replacement = fixture("green.png");
    let mip = format!("1={}", replacement.display());
    let (code, stdout, stderr) = run(&[
        "-d",
        &directory_arg(directory.path()),
        "-w",
        "64",
        "--mip",
        &mip,
        "-q",
        "fast",
        &source_arg(&source),
    ]);

    assert_eq!(0, code, "{stdout}{stderr}");
    let bytes = std::fs::read(directory.path().join("spray.vtf")).expect("read vtf");
    assert_eq!(2, bytes[56]);
    let level = 32u32;
    let decoded = decode_dxt1(&bytes, 64, (level / 4) as usize, level, level);
    let pixel = decoded[((level / 2) * level + level / 2) as usize];
    assert_eq!(255, (pixel >> 8) & 0xff, "the replacement image is green");
    assert_eq!(0, pixel & 0xff);
}

#[test]
fn a_file_that_is_not_an_image_is_refused() {
    let directory = tempfile::tempdir().expect("temp dir");
    let broken = directory.path().join("broken.dat");
    std::fs::write(&broken, "not an image at all").expect("write");
    let (code, _stdout, stderr) = run(&[
        "-d",
        &directory_arg(directory.path()),
        "-w",
        "16",
        &source_arg(&broken),
    ]);

    assert_eq!(1, code);
    assert!(stderr.contains("broken.dat"), "{stderr}");
}

#[test]
fn an_invalid_output_name_is_refused() {
    let directory = tempfile::tempdir().expect("temp dir");
    let source = fixture("sample.png");
    let (code, _stdout, stderr) = run(&[
        "-d",
        &directory_arg(directory.path()),
        "-o",
        "bad/name",
        &source_arg(&source),
    ]);

    assert_eq!(1, code);
    assert!(stderr.contains("output name"), "{stderr}");
}

#[test]
fn a_job_count_below_one_is_refused() {
    let directory = tempfile::tempdir().expect("temp dir");
    let source = fixture("strip.png");
    let (code, _stdout, stderr) = run(&[
        "-d",
        &directory_arg(directory.path()),
        "--strip",
        "-w",
        "16",
        "-j",
        "0",
        &source_arg(&source),
    ]);

    assert_eq!(1, code);
    assert!(stderr.contains("--jobs"), "{stderr}");
}

#[test]
fn a_video_is_imported_through_ffmpeg() {
    if !ffmpeg_available() {
        eprintln!("skipping: ffmpeg is not installed");
        return;
    }
    let directory = tempfile::tempdir().expect("temp dir");
    let clip = directory.path().join("clip.mp4");
    let created = Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc=size=32x32:rate=10",
            "-t",
            "1",
            "-pix_fmt",
            "yuv420p",
            "-y",
        ])
        .arg(&clip)
        .output()
        .expect("run ffmpeg");
    if !created.status.success() {
        eprintln!("skipping: ffmpeg could not make a test clip");
        return;
    }

    let (code, stdout, stderr) = run(&[
        "-d",
        &directory_arg(directory.path()),
        "-w",
        "32",
        &source_arg(&clip),
    ]);

    assert_eq!(0, code, "{stdout}{stderr}");
    let bytes = std::fs::read(directory.path().join("spray.vtf")).expect("read vtf");
    let frames = u16_at(&bytes, 24);
    assert!(
        frames >= 4,
        "a second at five frames a second should give about five frames, got {frames}"
    );
    assert_ne!(0, u32_at(&bytes, 52));
}

#[test]
fn a_forced_progress_bar_is_drawn_on_stderr_before_the_result_summary() {
    let directory = tempfile::tempdir().expect("temp dir");
    let source = fixture("sample.png");
    let (code, stdout, stderr) = run(&[
        "-d",
        &directory_arg(directory.path()),
        "-w",
        "32",
        "-q",
        "fast",
        "--progress",
        &source_arg(&source),
    ]);

    assert_eq!(0, code, "{stdout}{stderr}");
    assert!(
        stderr.contains("encoding  [") && stderr.contains("100%"),
        "no bar was drawn\n{stderr}"
    );
    assert!(
        stdout.contains("spray.vtf  "),
        "no summary was printed\n{stdout}"
    );
}

#[test]
fn no_progress_never_draws_a_bar() {
    let directory = tempfile::tempdir().expect("temp dir");
    let source = fixture("sample.png");
    let (code, stdout, stderr) = run(&[
        "-d",
        &directory_arg(directory.path()),
        "-w",
        "32",
        "-q",
        "fast",
        "--no-progress",
        &source_arg(&source),
    ]);

    assert_eq!(0, code, "{stdout}{stderr}");
    assert!(!stderr.contains("encoding  ["), "{stderr}");
}

#[test]
fn the_default_run_draws_no_bar_when_there_is_no_console() {
    let directory = tempfile::tempdir().expect("temp dir");
    let source = fixture("sample.png");
    let (code, stdout, stderr) = run(&[
        "-d",
        &directory_arg(directory.path()),
        "-w",
        "32",
        "-q",
        "fast",
        &source_arg(&source),
    ]);

    assert_eq!(0, code, "{stdout}{stderr}");
    assert!(!stderr.contains("encoding  ["), "{stderr}");
}

fn run(args: &[&str]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_leake"))
        .args(args)
        .output()
        .expect("run leake");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

fn directory_arg(directory: &Path) -> String {
    directory.to_str().expect("utf-8 path").to_owned()
}

fn source_arg(path: &Path) -> String {
    path.to_str().expect("utf-8 path").to_owned()
}

fn signature(bytes: &[u8]) -> &str {
    std::str::from_utf8(&bytes[0..3]).expect("signature")
}

fn u16_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from(bytes[offset]) | (u32::from(bytes[offset + 1]) << 8)
}

fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

fn ffmpeg_available() -> bool {
    Command::new("ffmpeg")
        .arg("-version")
        .output()
        .is_ok_and(|output| output.status.success())
}

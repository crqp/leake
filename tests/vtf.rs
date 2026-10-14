mod common;

use leake::budget::Layout;
use leake::format::{Sampling, VtfFormat};
use leake::vtf;

fn layout() -> Layout {
    Layout {
        width: 256,
        height: 256,
        frames: 1,
        shortened: false,
        mipmaps: true,
        mipmaps_permitted: true,
        mip_count: 1,
    }
}

fn levels() -> Vec<Vec<u8>> {
    vec![vec![0u8; 256 * 256 / 2], vec![0u8; 128 * 128 / 2]]
}

#[test]
fn header_fields_are_where_the_format_puts_them() {
    let file = vtf::write(&layout(), VtfFormat::Dxt1, Sampling::Default, &levels());

    assert_eq!([0x56, 0x54, 0x46, 0x00], file[0..4]);
    assert_eq!(7, little_endian32(&file, 4), "major version");
    assert_eq!(1, little_endian32(&file, 8), "minor version");
    assert_eq!(64, little_endian32(&file, 12), "header size");
    assert_eq!(256, little_endian16(&file, 16), "width");
    assert_eq!(256, little_endian16(&file, 18), "height");
    assert_eq!(1, little_endian16(&file, 24), "frame count");
    assert_eq!(0, little_endian16(&file, 26), "first frame");
    assert_eq!(VtfFormat::Dxt1.id(), little_endian32(&file, 52), "format");
    assert_eq!(2, file[56], "mipmap count includes the base level");
    assert_eq!(
        VtfFormat::Dxt1.id(),
        little_endian32(&file, 57),
        "low resolution image format"
    );
    assert_eq!(0, file[61], "low resolution image width");
    assert_eq!(0, file[62], "low resolution image height");
}

#[test]
fn flags_describe_how_the_texture_is_sampled() {
    let clamped = vtf::write(&layout(), VtfFormat::Dxt1, Sampling::Default, &levels());
    let flags = little_endian32(&clamped, 20);
    assert_eq!(4 | 8, flags & 0x0c, "both axes should be clamped");
    assert_eq!(0, flags & 0x01, "default sampling is not point sampling");
    assert_eq!(
        0,
        flags & 0x100,
        "a texture with mipmaps should not say it has none"
    );

    let point = vtf::write(&layout(), VtfFormat::Dxt1, Sampling::Point, &levels());
    assert_eq!(
        1,
        little_endian32(&point, 20) & 0x01,
        "point sampling should be flagged"
    );

    let no_mipmaps = Layout {
        mipmaps: false,
        mip_count: 0,
        ..layout()
    };
    let bare = vtf::write(
        &no_mipmaps,
        VtfFormat::Dxt1,
        Sampling::Default,
        &levels()[..1],
    );
    assert_eq!(
        0x100,
        little_endian32(&bare, 20) & 0x100,
        "a texture without mipmaps should say so"
    );
}

#[test]
fn levels_are_stored_smallest_first() {
    let base = vec![0x11u8; 8];
    let smaller = vec![0x22u8; 8];
    let file = vtf::write(
        &layout(),
        VtfFormat::Dxt1,
        Sampling::Default,
        &[base.clone(), smaller.clone()],
    );

    assert_eq!(smaller, file[64..72]);
    assert_eq!(base, file[72..80]);
}

fn little_endian16(bytes: &[u8], offset: usize) -> u32 {
    u32::from(bytes[offset]) | (u32::from(bytes[offset + 1]) << 8)
}

fn little_endian32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

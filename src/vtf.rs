use std::fmt::Write as _;

use crate::budget::Layout;
use crate::format::{Sampling, VtfFormat};

pub const HEADER_SIZE: usize = 64;

const SIGNATURE: u32 = 0x0046_5456;

const VERSION_MAJOR: u32 = 7;
const VERSION_MINOR: u32 = 1;

const FLAG_CLAMPS: u32 = 0x0000_0004;
const FLAG_CLAMPT: u32 = 0x0000_0008;
const FLAG_HINT_DXT5: u32 = 0x0000_0020;
const FLAG_NOMIP: u32 = 0x0000_0100;
const FLAG_NOLOD: u32 = 0x0000_0200;

const OFFSET_SIGNATURE: usize = 0;
const OFFSET_VERSION: usize = 4;
const OFFSET_HEADER_SIZE: usize = 12;
const OFFSET_WIDTH: usize = 16;
const OFFSET_HEIGHT: usize = 18;
const OFFSET_FLAGS: usize = 20;
const OFFSET_FRAMES: usize = 24;
const OFFSET_FIRST_FRAME: usize = 26;
const OFFSET_FORMAT: usize = 52;
const OFFSET_MIP_COUNT: usize = 56;
const OFFSET_LOW_RES_FORMAT: usize = 57;
const OFFSET_LOW_RES_WIDTH: usize = 61;
const OFFSET_LOW_RES_HEIGHT: usize = 62;
const OFFSET_DEPTH: usize = 63;

const DEPTH: u8 = 1;

#[must_use]
pub fn write(
    layout: &Layout,
    format: VtfFormat,
    sampling: Sampling,
    levels: &[Vec<u8>],
) -> Vec<u8> {
    let size = HEADER_SIZE + levels.iter().map(Vec::len).sum::<usize>();
    let mut file = vec![0u8; size];
    write_int(&mut file, OFFSET_SIGNATURE, SIGNATURE);
    write_int(&mut file, OFFSET_VERSION, VERSION_MAJOR);
    write_int(&mut file, OFFSET_VERSION + 4, VERSION_MINOR);
    write_int(&mut file, OFFSET_HEADER_SIZE, HEADER_SIZE as u32);
    write_short(&mut file, OFFSET_WIDTH, layout.width);
    write_short(&mut file, OFFSET_HEIGHT, layout.height);
    write_int(&mut file, OFFSET_FLAGS, flags(layout, format, sampling));
    write_short(&mut file, OFFSET_FRAMES, layout.frames);
    write_short(&mut file, OFFSET_FIRST_FRAME, 0);
    write_int(&mut file, OFFSET_FORMAT, format.id());
    file[OFFSET_MIP_COUNT] = levels.len() as u8;
    write_int(&mut file, OFFSET_LOW_RES_FORMAT, VtfFormat::Dxt1.id());
    file[OFFSET_LOW_RES_WIDTH] = 0;
    file[OFFSET_LOW_RES_HEIGHT] = 0;
    file[OFFSET_DEPTH] = DEPTH;

    let mut offset = HEADER_SIZE;
    for level in levels.iter().rev() {
        file[offset..offset + level.len()].copy_from_slice(level);
        offset += level.len();
    }
    file
}

fn flags(layout: &Layout, format: VtfFormat, sampling: Sampling) -> u32 {
    let mut flags = FLAG_CLAMPS | FLAG_CLAMPT | FLAG_NOLOD | sampling.flag();
    if format.is_dxt() {
        flags |= FLAG_HINT_DXT5;
    }
    if !layout.mipmaps {
        flags |= FLAG_NOMIP;
    }
    flags
}

#[must_use]
pub fn vmt(name: &str) -> String {
    let mut material = String::new();
    material.push_str("\"UnlitGeneric\"\n");
    material.push_str("{\n");
    let _ = writeln!(material, "\t\"$basetexture\"\t\"vgui/logos/{name}\"");
    material.push_str("\t\"$translucent\" \"1\"\n");
    material.push_str("\t\"$ignorez\" \"1\"\n");
    material.push_str("\t\"$vertexcolor\" \"1\"\n");
    material.push_str("\t\"$vertexalpha\" \"1\"\n");
    material.push_str("}\n");
    material
}

fn write_short(target: &mut [u8], offset: usize, value: u32) {
    target[offset] = (value & 0xff) as u8;
    target[offset + 1] = ((value >> 8) & 0xff) as u8;
}

fn write_int(target: &mut [u8], offset: usize, value: u32) {
    for i in 0..4 {
        target[offset + i] = ((value >> (i * 8)) & 0xff) as u8;
    }
}

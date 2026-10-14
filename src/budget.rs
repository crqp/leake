use crate::error::{ImportError, Result};
use crate::format::{Sampling, VtfFormat};
use crate::vtf;

pub const LIMIT_KIB: f64 = 513.0;

const MIPMAP_FACTOR: f64 = 1.332_031_25;

const MIPMAP_LIMIT_KIB: f64 = 385.0;

const MIN_RESOLUTION: u32 = 4;

const MIN_MIPMAP_RESOLUTION: u32 = 16;

pub const MAX_ATLAS_HEIGHT: u32 = 32_767;

const MAX_WIDTH: u32 = 65_535;

#[must_use]
pub fn est_bytes(width: u32, height: u32, frames: u32, format: VtfFormat, mipmaps: bool) -> f64 {
    let texture = f64::from(width) * f64::from(height) * f64::from(frames) * format.multiplier();
    let texture = if mipmaps {
        texture * MIPMAP_FACTOR
    } else {
        texture
    };
    texture + vtf::HEADER_SIZE as f64
}

#[must_use]
pub fn est_kib(width: u32, height: u32, frames: u32, format: VtfFormat, mipmaps: bool) -> f64 {
    est_bytes(width, height, frames, format, mipmaps) / 1024.0
}

#[must_use]
pub fn clamp_resolution(source_width: u32, source_height: u32, frames: u32) -> u32 {
    let mut limit = source_width.max(source_height);
    limit = match frames {
        16_385.. => limit.min(4),
        4097..=16_384 => limit.min(8),
        1025..=4096 => limit.min(16),
        257..=1024 => limit.min(32),
        65..=256 => limit.min(64),
        17..=64 => limit.min(128),
        5..=16 => limit.min(256),
        2..=4 => limit.min(512),
        _ => limit,
    };
    let mut resolution = 4;
    while resolution < 1024 && resolution < limit {
        resolution *= 2;
    }
    resolution
}

#[must_use]
pub fn mip_count(width: u32, height: u32) -> u32 {
    let mut count = 0;
    let mut divisor = 2;
    while width / divisor > MIN_MIPMAP_RESOLUTION && height / divisor > MIN_MIPMAP_RESOLUTION {
        count += 1;
        divisor *= 2;
    }
    count
}

#[must_use]
pub fn mipmaps_allowed(
    output_width: u32,
    height: u32,
    frames: u32,
    format: VtfFormat,
    sampling: Sampling,
    shortened: bool,
) -> bool {
    if shortened || sampling == Sampling::Point {
        return false;
    }
    if !output_width.is_multiple_of(64) || !height.is_multiple_of(64) {
        return false;
    }
    est_kib(output_width, height, frames, format, false) < MIPMAP_LIMIT_KIB
}

fn shorten_if_borderline(width: u32, height: u32, frames: u32, format: VtfFormat) -> bool {
    if width
        .checked_sub(4)
        .is_none_or(|short| short < MIN_RESOLUTION)
    {
        return false;
    }
    let kib = est_kib(width, height, frames, format, false);
    (512.0..LIMIT_KIB).contains(&kib)
}

#[derive(Clone, Copy, Debug)]
pub struct LayoutRequest {
    pub source_width: u32,
    pub source_height: u32,
    pub frames: u32,
    pub format: VtfFormat,
    pub sampling: Sampling,
    pub requested_width: Option<u32>,
    pub requested_height: Option<u32>,
    pub mipmaps_requested: Option<bool>,
}

impl LayoutRequest {
    #[must_use]
    pub const fn new(source_width: u32, source_height: u32, frames: u32) -> Self {
        Self {
            source_width,
            source_height,
            frames,
            format: VtfFormat::Dxt1,
            sampling: Sampling::Default,
            requested_width: None,
            requested_height: None,
            mipmaps_requested: None,
        }
    }
}

pub fn resolve(request: &LayoutRequest) -> Result<Layout> {
    let LayoutRequest {
        source_width,
        source_height,
        frames,
        format,
        sampling,
        requested_width,
        requested_height,
        mipmaps_requested,
    } = *request;
    let mut width;
    let mut height;
    if let Some(requested) = requested_width.or(requested_height) {
        width = requested_width.unwrap_or(requested);
        height = requested_height.unwrap_or(requested);
    } else {
        width = clamp_resolution(source_width, source_height, frames);
        height = width;
        while width > MIN_RESOLUTION && height > MIN_RESOLUTION {
            let too_large = est_kib(width, height, frames, format, false) > LIMIT_KIB;
            if !too_large && u64::from(height) * u64::from(frames) <= u64::from(MAX_ATLAS_HEIGHT) {
                break;
            }
            width /= 2;
            height /= 2;
        }
    }

    if u64::from(height) * u64::from(frames) > u64::from(MAX_ATLAS_HEIGHT) {
        return Err(ImportError::new(format!(
            "cannot fit {frames} frames of {height}px into a {MAX_ATLAS_HEIGHT}px tall texture. Use -w to reduce the size, or import fewer frames"
        )));
    }
    if width > MAX_WIDTH {
        return Err(ImportError::new(format!(
            "a texture {width}px wide is wider than the {MAX_WIDTH}px limit. Use -w to reduce the size"
        )));
    }
    if est_kib(width, height, frames, format, false) > LIMIT_KIB {
        return Err(ImportError::new(format!(
            "a {width}x{height} texture of {frames} frames is too large for a spray. Use -w to reduce the size, or import fewer frames"
        )));
    }

    let shortened = shorten_if_borderline(width, height, frames, format);
    let output_width = if shortened { width - 4 } else { width };
    let permitted = mipmaps_allowed(output_width, height, frames, format, sampling, shortened);
    let mipmaps = mipmaps_requested.unwrap_or(permitted);
    Ok(Layout {
        width: output_width,
        height,
        frames,
        shortened,
        mipmaps,
        mipmaps_permitted: permitted,
        mip_count: if mipmaps {
            mip_count(output_width, height)
        } else {
            0
        },
    })
}

#[derive(Clone, Copy, Debug)]
pub struct Layout {
    pub width: u32,
    pub height: u32,
    pub frames: u32,
    pub shortened: bool,
    pub mipmaps: bool,
    pub mipmaps_permitted: bool,
    pub mip_count: u32,
}

pub struct FrameBudget {
    width: u32,
    height: u32,
    format: VtfFormat,
    frames: u32,
    pub truncated: bool,
}

impl FrameBudget {
    #[must_use]
    pub const fn new(width: u32, height: u32, format: VtfFormat) -> Self {
        Self {
            width,
            height,
            format,
            frames: 0,
            truncated: false,
        }
    }

    pub fn stop(&mut self) -> bool {
        self.frames += 1;
        if est_kib(self.width, self.height, self.frames, self.format, false) > LIMIT_KIB {
            self.truncated = true;
            true
        } else {
            false
        }
    }
}

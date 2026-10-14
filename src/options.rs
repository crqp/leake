use std::path::PathBuf;

use crate::format::{Quality, Sampling, VtfFormat};

#[derive(Clone, Debug)]
pub struct MipSpec {
    pub level: u32,
    pub paths: Vec<PathBuf>,
}

#[derive(Clone, Debug)]
#[allow(clippy::struct_excessive_bools)]
pub struct SprayOptions {
    pub inputs: Vec<PathBuf>,
    pub output: String,
    pub output_directory: PathBuf,
    pub format: VtfFormat,
    pub quality: Quality,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub mipmaps: Option<bool>,
    pub mip_specs: Vec<MipSpec>,
    pub dither: bool,
    pub sampling: Sampling,
    pub strip: bool,
    pub rescale: bool,
    pub speed: f64,
    pub start: f64,
    pub end: Option<f64>,
    pub all_frames: bool,
    pub write_vmt: bool,
    pub jobs: usize,
    pub progress: Option<bool>,
    pub dry_run: bool,
}

impl Default for SprayOptions {
    fn default() -> Self {
        Self {
            inputs: Vec::new(),
            output: "spray".to_owned(),
            output_directory: PathBuf::from("."),
            format: VtfFormat::Dxt1,
            quality: Quality::Slow,
            width: None,
            height: None,
            mipmaps: None,
            mip_specs: Vec::new(),
            dither: true,
            sampling: Sampling::Default,
            strip: false,
            rescale: true,
            speed: 1.0,
            start: 0.0,
            end: None,
            all_frames: false,
            write_vmt: true,
            jobs: default_jobs(),
            progress: None,
            dry_run: false,
        }
    }
}

#[must_use]
pub fn default_jobs() -> usize {
    std::thread::available_parallelism().map_or(1, std::num::NonZero::get)
}

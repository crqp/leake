use std::path::PathBuf;

use clap::Parser;

use crate::format::{Quality, Sampling, VtfFormat};
use crate::options::{MipSpec, SprayOptions, default_jobs};

#[derive(Debug, Parser)]
#[command(
    name = "leake",
    version = "1.0",
    about = "Converts images, animated GIFs, and videos into Source Engine spray textures."
)]
#[allow(clippy::struct_excessive_bools)]
pub struct Cli {
    /// images, or a single animated GIF or video
    #[arg(value_name = "INPUT", required = true, num_args = 1..)]
    pub inputs: Vec<PathBuf>,

    /// base name of the files to write
    #[arg(
        short = 'o',
        long = "output",
        value_name = "NAME",
        default_value = "spray"
    )]
    pub output: String,

    /// directory to write the files into
    #[arg(
        short = 'd',
        long = "output-dir",
        value_name = "DIR",
        default_value = "."
    )]
    pub output_directory: PathBuf,

    /// texture format
    #[arg(
        short = 'f',
        long = "format",
        value_enum,
        ignore_case = true,
        default_value_t = VtfFormat::Dxt1
    )]
    pub format: VtfFormat,

    /// DXT compression effort
    #[arg(
        short = 'q',
        long = "quality",
        value_enum,
        ignore_case = true,
        default_value_t = Quality::Slow
    )]
    pub quality: Quality,

    /// texture width, used for both sides when the height is left out
    #[arg(short = 'w', long = "width", value_name = "PIXELS")]
    pub width: Option<u32>,

    /// texture height
    #[arg(long = "height", value_name = "PIXELS")]
    pub height: Option<u32>,

    /// write mipmap levels
    #[arg(long = "mipmaps", overrides_with = "no_mipmaps")]
    pub mipmaps: bool,

    /// suppress mipmap levels
    #[arg(long = "no-mipmaps", overrides_with = "mipmaps")]
    pub no_mipmaps: bool,

    /// images for one mipmap level, repeatable
    #[arg(long = "mip", value_name = "LEVEL=FILES", value_parser = parse_mip)]
    pub mip: Vec<MipSpec>,

    /// dither when reducing to fewer colours
    #[arg(long = "dither", overrides_with = "no_dither")]
    pub dither: bool,

    /// suppress dithering
    #[arg(long = "no-dither", overrides_with = "dither")]
    pub no_dither: bool,

    /// sampling method
    #[arg(
        short = 's',
        long = "sampling",
        value_enum,
        ignore_case = true,
        default_value_t = Sampling::Default
    )]
    pub sampling: Sampling,

    /// cut a single tall image into animation frames
    #[arg(long = "strip")]
    pub strip: bool,

    /// scale each frame to fit its slot
    #[arg(long = "rescale", overrides_with = "no_rescale")]
    pub rescale: bool,

    /// leave each frame at its own size
    #[arg(long = "no-rescale", overrides_with = "rescale")]
    pub no_rescale: bool,

    /// playback speed for imported animations
    #[arg(long = "speed", value_name = "FACTOR", default_value_t = 1.0)]
    pub speed: f64,

    /// first video second or GIF frame to import
    #[arg(long = "start", value_name = "SECONDS", default_value_t = 0.0)]
    pub start: f64,

    /// last video second or GIF frame to import
    #[arg(long = "end", value_name = "SECONDS")]
    pub end: Option<f64>,

    /// import every GIF frame rather than sampling at spray playback rate
    #[arg(long = "all-frames")]
    pub all_frames: bool,

    /// write a .vmt material file beside the texture
    #[arg(long = "vmt", overrides_with = "no_vmt")]
    pub vmt: bool,

    /// suppress the .vmt material file
    #[arg(long = "no-vmt", overrides_with = "vmt")]
    pub no_vmt: bool,

    /// threads to encode with
    #[arg(short = 'j', long = "jobs", value_name = "COUNT", default_value_t = default_jobs())]
    pub jobs: usize,

    /// encode on the GPU; output may differ slightly from the CPU encoder
    #[arg(long = "gpu")]
    pub gpu: bool,

    /// show encoding progress on stderr
    #[arg(long = "progress", overrides_with = "no_progress")]
    pub progress: bool,

    /// suppress encoding progress
    #[arg(long = "no-progress", overrides_with = "progress")]
    pub no_progress: bool,

    /// report the layout that would be produced, without writing files
    #[arg(long = "dry-run")]
    pub dry_run: bool,
}

impl Cli {
    #[must_use]
    pub fn into_options(self) -> SprayOptions {
        let Cli {
            inputs,
            output,
            output_directory,
            format,
            quality,
            width,
            height,
            mipmaps,
            no_mipmaps,
            mip,
            dither,
            no_dither,
            sampling,
            strip,
            rescale,
            no_rescale,
            speed,
            start,
            end,
            all_frames,
            vmt,
            no_vmt,
            jobs,
            gpu,
            progress,
            no_progress,
            dry_run,
        } = self;
        SprayOptions {
            inputs,
            output,
            output_directory,
            format,
            quality,
            width,
            height,
            mipmaps: toggle(mipmaps, no_mipmaps),
            mip_specs: mip,
            dither: toggle(dither, no_dither).unwrap_or(true),
            sampling,
            strip,
            rescale: toggle(rescale, no_rescale).unwrap_or(true),
            speed,
            start,
            end,
            all_frames,
            write_vmt: toggle(vmt, no_vmt).unwrap_or(true),
            jobs,
            gpu,
            progress: toggle(progress, no_progress),
            dry_run,
        }
    }
}

// leaving None when neither flag appears lets the caller apply its own default
fn toggle(yes: bool, no: bool) -> Option<bool> {
    match (yes, no) {
        (true, _) => Some(true),
        (false, true) => Some(false),
        (false, false) => None,
    }
}

fn parse_mip(value: &str) -> Result<MipSpec, String> {
    let Some((level, files)) = value.split_once('=') else {
        return Err(MIP_ERROR.to_owned());
    };
    let paths = files
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    let level = level.trim().parse::<u32>().ok().filter(|level| *level >= 1);
    match level {
        Some(level) if !paths.is_empty() => Ok(MipSpec { level, paths }),
        _ => Err(MIP_ERROR.to_owned()),
    }
}

const MIP_ERROR: &str = "--mip takes LEVEL=FILE[,FILE...], for example --mip 2=faded.png";

use std::fmt;

use clap::ValueEnum;

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VtfFormat {
    Rgba8888,
    Rgb888,
    Rgb565,
    Bgra4444,
    Bgra5551,
    Dxt1,
    Dxt5,
}

impl VtfFormat {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Rgba8888 => "RGBA8888",
            Self::Rgb888 => "RGB888",
            Self::Rgb565 => "RGB565",
            Self::Bgra4444 => "BGRA4444",
            Self::Bgra5551 => "BGRA5551",
            Self::Dxt1 => "DXT1",
            Self::Dxt5 => "DXT5",
        }
    }

    #[must_use]
    pub const fn id(self) -> u32 {
        match self {
            Self::Rgba8888 => 0,
            Self::Rgb888 => 2,
            Self::Rgb565 => 4,
            Self::Bgra4444 => 19,
            Self::Bgra5551 => 21,
            Self::Dxt1 => 13,
            Self::Dxt5 => 15,
        }
    }

    #[must_use]
    pub const fn multiplier(self) -> f64 {
        match self {
            Self::Rgba8888 => 4.0,
            Self::Rgb888 => 3.0,
            Self::Rgb565 | Self::Bgra4444 | Self::Bgra5551 => 2.0,
            Self::Dxt1 => 0.5,
            Self::Dxt5 => 1.0,
        }
    }

    #[must_use]
    pub const fn is_dxt(self) -> bool {
        matches!(self, Self::Dxt1 | Self::Dxt5)
    }

    #[must_use]
    pub const fn bytes_per_block(self) -> usize {
        if matches!(self, Self::Dxt5) { 16 } else { 8 }
    }
}

impl fmt::Display for VtfFormat {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.name())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Sampling {
    #[default]
    Default,
    Point,
    Trilinear,
    Anisotropic,
}

impl Sampling {
    #[must_use]
    pub const fn flag(self) -> u32 {
        match self {
            Self::Default => 0,
            Self::Point => 1,
            Self::Trilinear => 2,
            Self::Anisotropic => 16,
        }
    }
}

impl fmt::Display for Sampling {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.name())
    }
}

impl Sampling {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Default => "DEFAULT",
            Self::Point => "POINT",
            Self::Trilinear => "TRILINEAR",
            Self::Anisotropic => "ANISOTROPIC",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Speed {
    Fast,
    Normal,
}

#[derive(Clone, Copy, Debug)]
pub struct DxtConfig {
    pub rgb_steps: u32,
    pub alpha_steps: usize,
    pub use_3d_refinement: bool,
    pub adaptive_weighting: bool,
    pub colour_speed: Speed,
    pub alpha_speed: Speed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Quality {
    Fast,
    Normal,
    Slow,
    VerySlow,
}

impl Quality {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Fast => "FAST",
            Self::Normal => "NORMAL",
            Self::Slow => "SLOW",
            Self::VerySlow => "VERY_SLOW",
        }
    }

    #[must_use]
    pub const fn config(self) -> DxtConfig {
        match self {
            Self::Fast => DxtConfig {
                rgb_steps: 0,
                alpha_steps: 1,
                use_3d_refinement: false,
                adaptive_weighting: false,
                colour_speed: Speed::Fast,
                alpha_speed: Speed::Normal,
            },
            Self::Normal => DxtConfig {
                rgb_steps: 1,
                alpha_steps: 2,
                use_3d_refinement: false,
                adaptive_weighting: false,
                colour_speed: Speed::Normal,
                alpha_speed: Speed::Fast,
            },
            Self::Slow => DxtConfig {
                rgb_steps: 2,
                alpha_steps: 3,
                use_3d_refinement: false,
                adaptive_weighting: true,
                colour_speed: Speed::Normal,
                alpha_speed: Speed::Normal,
            },
            Self::VerySlow => DxtConfig {
                rgb_steps: 2,
                alpha_steps: 4,
                use_3d_refinement: true,
                adaptive_weighting: true,
                colour_speed: Speed::Normal,
                alpha_speed: Speed::Normal,
            },
        }
    }
}

impl fmt::Display for Quality {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.name())
    }
}

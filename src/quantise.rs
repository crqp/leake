use std::collections::HashMap;
use std::sync::LazyLock;

use crate::format::VtfFormat;

const LADDER_STEPS: usize = 17;

const DITHER_SIZE: usize = 4;

const CHANNELS: usize = 3;

// rows index the 17 rungs of the ladder between the two nearest shades, columns index a 4x4 ordered pattern across the image
pub(crate) const DITHER_MATRIX: [[u8; DITHER_SIZE * DITHER_SIZE]; LADDER_STEPS] = [
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0],
    [1, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0],
    [1, 0, 1, 0, 0, 0, 0, 0, 1, 0, 1, 0, 0, 0, 0, 0],
    [1, 0, 1, 0, 0, 1, 0, 0, 1, 0, 1, 0, 0, 0, 0, 0],
    [1, 0, 1, 0, 0, 1, 0, 0, 1, 0, 1, 0, 0, 0, 0, 1],
    [1, 0, 1, 0, 0, 1, 0, 1, 1, 0, 1, 0, 0, 0, 0, 1],
    [1, 0, 1, 0, 0, 1, 0, 1, 1, 0, 1, 0, 0, 1, 0, 1],
    [1, 1, 1, 0, 0, 1, 0, 1, 1, 0, 1, 0, 0, 1, 0, 1],
    [1, 1, 1, 0, 0, 1, 0, 1, 1, 0, 1, 1, 0, 1, 0, 1],
    [1, 1, 1, 1, 0, 1, 0, 1, 1, 0, 1, 1, 0, 1, 0, 1],
    [1, 1, 1, 1, 0, 1, 0, 1, 1, 1, 1, 1, 0, 1, 0, 1],
    [1, 1, 1, 1, 1, 1, 0, 1, 1, 1, 1, 1, 0, 1, 0, 1],
    [1, 1, 1, 1, 1, 1, 0, 1, 1, 1, 1, 1, 0, 1, 1, 1],
    [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 1, 1, 1],
    [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
];

#[derive(Clone, Copy)]
pub struct FormatSpec {
    pub red: u32,
    pub green: u32,
    pub blue: u32,
    pub alpha: u32,
    pub opaque: bool,
    pub dithers: bool,
}

impl FormatSpec {
    #[must_use]
    pub const fn widths(self) -> [u32; CHANNELS] {
        [self.red, self.green, self.blue]
    }

    #[must_use]
    pub const fn true_colour(self) -> bool {
        self.red == 8 && self.green == 8 && self.blue == 8
    }

    #[must_use]
    pub const fn identity(self) -> bool {
        self.true_colour() && self.alpha == 8 && !self.opaque
    }
}

#[must_use]
pub fn spec(format: VtfFormat) -> FormatSpec {
    match format {
        VtfFormat::Rgba8888 => FormatSpec {
            red: 8,
            green: 8,
            blue: 8,
            alpha: 8,
            opaque: false,
            dithers: false,
        },
        VtfFormat::Rgb888 => FormatSpec {
            red: 8,
            green: 8,
            blue: 8,
            alpha: 8,
            opaque: true,
            dithers: false,
        },
        VtfFormat::Rgb565 => FormatSpec {
            red: 5,
            green: 6,
            blue: 5,
            alpha: 8,
            opaque: true,
            dithers: true,
        },
        VtfFormat::Bgra5551 => FormatSpec {
            red: 5,
            green: 5,
            blue: 5,
            alpha: 1,
            opaque: false,
            dithers: true,
        },
        VtfFormat::Bgra4444 => FormatSpec {
            red: 4,
            green: 4,
            blue: 4,
            alpha: 4,
            opaque: false,
            dithers: true,
        },
        VtfFormat::Dxt1 | VtfFormat::Dxt5 => unreachable!("DXT formats are not packed"),
    }
}

struct Shades {
    lower: [f64; 256],
    upper: [f64; 256],
}

impl Shades {
    fn new(bits: u32) -> Self {
        let scale = step_scale(bits);
        let mut lower = [0.0; 256];
        let mut upper = [0.0; 256];
        for value in 0..256usize {
            lower[value] = lower_shade(value, scale);
            upper[value] = upper_shade(value, scale);
        }
        Self { lower, upper }
    }
}

static SHADES: LazyLock<HashMap<u32, Shades>> = LazyLock::new(|| {
    HashMap::from([
        (1, Shades::new(1)),
        (4, Shades::new(4)),
        (5, Shades::new(5)),
        (6, Shades::new(6)),
    ])
});

fn shades(bits: u32) -> &'static Shades {
    SHADES.get(&bits).expect("known shade width")
}

#[cfg(feature = "gpu")]
#[must_use]
pub(crate) fn shade_tables(bits: u32) -> ([u32; 256], [u32; 256]) {
    let shades = shades(bits);
    let mut lower = [0u32; 256];
    let mut upper = [0u32; 256];
    for value in 0..256 {
        lower[value] = shades.lower[value] as u32;
        upper[value] = shades.upper[value] as u32;
    }
    (lower, upper)
}

#[must_use]
pub fn reducer(format: VtfFormat, dither: bool) -> Reducer {
    Reducer::new(spec(format), dither)
}

pub struct Reducer {
    spec: FormatSpec,
    dithers: bool,
    source: [i32; CHANNELS],
    lower: [f64; CHANNELS],
    upper: [f64; CHANNELS],
    ladder: [f64; LADDER_STEPS * CHANNELS],
    lower_shades: Option<[&'static Shades; CHANNELS]>,
    upper_shades: Option<[&'static Shades; CHANNELS]>,
    alpha_shades: Option<&'static Shades>,
}

impl Reducer {
    #[must_use]
    pub fn new(spec: FormatSpec, dither: bool) -> Self {
        let widths = spec.widths();
        let lower_shades = if spec.true_colour() {
            None
        } else {
            Some([shades(widths[0]), shades(widths[1]), shades(widths[2])])
        };
        let upper_shades = if spec.true_colour() {
            None
        } else {
            Some([shades(widths[0]), shades(widths[1]), shades(widths[2])])
        };
        let alpha_shades = if spec.opaque || spec.alpha >= 8 {
            None
        } else {
            Some(shades(spec.alpha))
        };
        Self {
            spec,
            dithers: dither && spec.dithers,
            source: [0; CHANNELS],
            lower: [0.0; CHANNELS],
            upper: [0.0; CHANNELS],
            ladder: [0.0; LADDER_STEPS * CHANNELS],
            lower_shades,
            upper_shades,
            alpha_shades,
        }
    }

    pub fn reduce(&mut self, pixel: u32, x: u32, y: u32) -> u32 {
        if self.spec.identity() {
            return pixel;
        }
        let alpha = if self.spec.opaque {
            255
        } else {
            self.reduce_alpha(((pixel >> 24) & 0xff) as i32)
        };
        let red = ((pixel >> 16) & 0xff) as i32;
        let green = ((pixel >> 8) & 0xff) as i32;
        let blue = (pixel & 0xff) as i32;
        let colour = if self.spec.true_colour() {
            ((red as u32) << 16) | ((green as u32) << 8) | blue as u32
        } else {
            self.reduce_colour(red, green, blue, x, y)
        };
        ((alpha as u32) << 24) | colour
    }

    fn reduce_colour(&mut self, red: i32, green: i32, blue: i32, x: u32, y: u32) -> u32 {
        let lower_table = self.lower_shades.expect("packed colour format");
        let upper_table = self.upper_shades.expect("packed colour format");
        self.source[0] = red;
        self.source[1] = green;
        self.source[2] = blue;

        let mut lower_distance = 0.0;
        let mut upper_distance = 0.0;
        for channel in 0..CHANNELS {
            let value = self.source[channel];
            let low = lower_table[channel].lower[value as usize];
            let high = upper_table[channel].upper[value as usize];
            self.lower[channel] = low;
            self.upper[channel] = high;
            lower_distance += (f64::from(value) - low).abs();
            upper_distance += (f64::from(value) - high).abs();
        }

        let near_is_lower = lower_distance <= upper_distance;
        let near = if near_is_lower {
            self.lower
        } else {
            self.upper
        };
        let far = if near_is_lower {
            self.upper
        } else {
            self.lower
        };
        if !self.dithers {
            return pack_channels(&near);
        }

        for rung in 0..LADDER_STEPS {
            for channel in 0..CHANNELS {
                self.ladder[rung * CHANNELS + channel] = near[channel]
                    + (far[channel] - near[channel]) * rung as f64 / (LADDER_STEPS - 1) as f64;
            }
        }

        let mut best_rung = 0;
        let mut best_distance = f64::MAX;
        for rung in 0..LADDER_STEPS {
            let mut distance = 0.0;
            for channel in 0..CHANNELS {
                distance += (self.ladder[rung * CHANNELS + channel]
                    - f64::from(self.source[channel]))
                .abs();
            }
            if distance < best_distance {
                best_distance = distance;
                best_rung = rung;
            }
        }

        let column =
            (y % DITHER_SIZE as u32) as usize * DITHER_SIZE + (x % DITHER_SIZE as u32) as usize;
        pack_channels(if DITHER_MATRIX[best_rung][column] == 1 {
            &far
        } else {
            &near
        })
    }

    fn reduce_alpha(&mut self, value: i32) -> i32 {
        let Some(shades) = self.alpha_shades else {
            return value;
        };
        let low = shades.lower[value as usize];
        let high = shades.upper[value as usize];
        if (f64::from(value) - low).abs() > (f64::from(value) - high).abs() {
            clamp_to_byte(high) as i32
        } else {
            clamp_to_byte(low) as i32
        }
    }
}

fn step_scale(bits: u32) -> f64 {
    255.0 / f64::from(255 >> (8 - bits))
}

fn lower_shade(value: usize, scale: f64) -> f64 {
    ((value as f64 / scale).floor() * scale + 0.5).floor()
}

fn upper_shade(value: usize, scale: f64) -> f64 {
    ((value as f64 / scale).ceil() * scale + 0.5).floor()
}

fn pack_channels(channels: &[f64; CHANNELS]) -> u32 {
    (clamp_to_byte(channels[0]) << 16)
        | (clamp_to_byte(channels[1]) << 8)
        | clamp_to_byte(channels[2])
}

// round exact halves to even so values sitting on a half do not systematically round up
fn clamp_to_byte(value: f64) -> u32 {
    if value <= 0.0 {
        return 0;
    }
    if value >= 255.0 {
        return 255;
    }
    let whole = value.floor();
    let fraction = value - whole;
    let rounded = if fraction < 0.5 {
        whole
    } else if fraction > 0.5 {
        whole + 1.0
    } else if whole % 2.0 == 0.0 {
        whole
    } else {
        whole + 1.0
    };
    rounded as u32
}

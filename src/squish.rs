#![allow(
    clippy::comparison_chain,
    clippy::float_cmp,
    clippy::if_not_else,
    clippy::manual_midpoint,
    clippy::needless_range_loop,
    clippy::too_many_arguments,
    clippy::too_many_lines
)]

mod alpha;
mod colour;
mod ramp;
mod refine;
mod search;

pub use alpha::compress_alpha_block;
pub use colour::compress_rgb_block;

const MAX_BLOCK: usize = 16;
const MAX_POINTS: usize = 16;
const NUM_ENDPOINTS: usize = 2;

const BC: usize = 0;
const GC: usize = 1;
const RC: usize = 2;
const AC: usize = 3;

const B_BIT_WIDTH: u32 = 5;
const G_BIT_WIDTH: u32 = 6;
const R_BIT_WIDTH: u32 = 5;

// colour spread below this threshold is treated as flat since it's too small to change quantised output
const EPS: f64 = (2.0 / 255.0) * (2.0 / 255.0);
const EPS2: f64 = 3.0 * (2.0 / 255.0) * (2.0 / 255.0);
const MAX_ERROR: f64 = 128_000.0;
const ALPHA_RANGE: f64 = 256.0;

const GLOBAL_SEARCH_STEP: f64 = 0.018;
const GLOBAL_SEARCH_EXTENT: f64 = 0.1;
const LOCAL_SEARCH_STEP: f64 = 0.6;

// red and blue use 5-bit codes while green uses 6-bit codes, so make it take finer steps and a larger factor
const GRID_FACTOR: [f64; 3] = [8.0, 4.0, 8.0];

const CHANNEL_BIT_WIDTHS: [u32; 3] = [B_BIT_WIDTH, G_BIT_WIDTH, R_BIT_WIDTH];

const REPLICATE_FACTOR: [f64; 3] = [32.0, 64.0, 32.0];

const BASE_WEIGHTS: [f64; 3] = [0.0820, 0.6094, 0.3086];

const REFINE_MOVES: [f64; 17] = [
    0.0, -1.0, 1.0, -2.0, 2.0, -3.0, 3.0, -4.0, 4.0, -5.0, 5.0, -6.0, 6.0, -7.0, 7.0, -8.0, 8.0,
];

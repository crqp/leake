use crate::format::DxtConfig;

use super::colour::endpoint_colour;
use super::refine::finish_ramp;
use super::search::{endpoint_error, find_axis_candidates, search_direction};
use super::{AC, BC, CHANNEL_BIT_WIDTHS, GC, MAX_BLOCK, MAX_POINTS, NUM_ENDPOINTS, RC};

pub(super) struct Ramp {
    pub(super) error: Option<f64>,
    pub(super) three_point: bool,
}

impl Ramp {
    pub(super) fn usable(&self) -> bool {
        self.error.is_some()
    }
}

pub(super) struct RampScratch {
    pub(super) projected: [f64; MAX_BLOCK],
    pub(super) raw_projection: [f64; MAX_BLOCK],
    pub(super) projection_error: [f64; MAX_BLOCK],
    pub(super) weighted_repeats: [f64; MAX_BLOCK],
    pub(super) ramp_indices: [f64; MAX_BLOCK],
    pub(super) bounds: [f64; 2],
    pub(super) endpoints: [f64; NUM_ENDPOINTS],
    pub(super) snapped: [f64; 3 * NUM_ENDPOINTS],
    pub(super) error_ramp: [f64; 4 * MAX_POINTS],
}

impl RampScratch {
    pub(super) fn new() -> Self {
        Self {
            projected: [0.0; MAX_BLOCK],
            raw_projection: [0.0; MAX_BLOCK],
            projection_error: [0.0; MAX_BLOCK],
            weighted_repeats: [0.0; MAX_BLOCK],
            ramp_indices: [0.0; MAX_BLOCK],
            bounds: [0.0; 2],
            endpoints: [0.0; NUM_ENDPOINTS],
            snapped: [0.0; 3 * NUM_ENDPOINTS],
            error_ramp: [0.0; 4 * MAX_POINTS],
        }
    }
}

pub(super) fn compress_ramp(
    block: &[i32; MAX_BLOCK],
    num_points: usize,
    config: &DxtConfig,
    weights: &[f64; 3],
    use_alpha: bool,
    alpha_threshold: i32,
    endpoints: &mut [u8; 3 * NUM_ENDPOINTS],
    indices: &mut [u8; MAX_BLOCK],
) -> Ramp {
    let mut colour_count = 0;
    let mut packed = [0i32; MAX_BLOCK];
    for value in block {
        if !use_alpha || ((*value as u32 >> 24) & 0xff) as i32 >= alpha_threshold {
            packed[colour_count] = *value | 0xff00_0000u32 as i32;
            colour_count += 1;
        }
    }
    if colour_count == 0 {
        endpoints[0] = 0;
        endpoints[1] = 0xff;
        endpoints[2] = 0;
        endpoints[3] = 0xff;
        endpoints[4] = 0;
        endpoints[5] = 0xff;
        indices.fill(0xff);
        return Ramp {
            error: Some(0.0),
            three_point: true,
        };
    }

    let mut sorted = packed;
    sorted[..colour_count].sort_unstable();

    let mut unique = [0i32; MAX_BLOCK];
    let mut repeats = [0.0f64; MAX_BLOCK];
    let mut unique_count = 1;
    unique[0] = sorted[0];
    repeats[0] = 1.0;
    for index in 1..colour_count {
        if sorted[index] != unique[unique_count - 1] {
            unique[unique_count] = sorted[index];
            repeats[unique_count] = 1.0;
            unique_count += 1;
        } else {
            repeats[unique_count - 1] += 1.0;
        }
    }

    if colour_count != MAX_BLOCK && use_alpha && num_points.is_multiple_of(2) {
        return Ramp {
            error: None,
            three_point: false,
        };
    }

    let mut input = [0.0f64; MAX_BLOCK * 4];
    for index in 0..unique_count {
        input[index * 4 + RC] = f64::from((unique[index] as u32 >> 16) & 0xff);
        input[index * 4 + GC] = f64::from((unique[index] as u32 >> 8) & 0xff);
        input[index * 4 + BC] = f64::from(unique[index] as u32 & 0xff);
        input[index * 4 + AC] = 255.0;
    }

    let mut result = [0.0f64; 3 * NUM_ENDPOINTS];
    solve_ramp(
        &mut result,
        &input,
        &repeats,
        unique_count,
        num_points,
        config,
        weights,
    );
    for (endpoint, value) in endpoints.iter_mut().zip(result.iter()) {
        *endpoint = (*value as i32 & 0xff) as u8;
    }
    let error = cluster_colour_block(
        block,
        *endpoints,
        indices,
        num_points,
        weights,
        use_alpha,
        alpha_threshold,
    );
    Ramp {
        error: Some(error),
        three_point: num_points == 3,
    }
}

fn solve_ramp(
    result: &mut [f64; 3 * NUM_ENDPOINTS],
    input: &[f64; MAX_BLOCK * 4],
    repeats: &[f64; MAX_BLOCK],
    unique_count: usize,
    num_points: usize,
    config: &DxtConfig,
    weights: &[f64; 3],
) {
    let mut normalised = [0.0f64; MAX_BLOCK * 4];
    for index in 0..unique_count {
        for channel in 0..3 {
            normalised[index * 4 + channel] = input[index * 4 + channel] / 255.0;
        }
    }

    if unique_count <= 2 {
        use_extremes(result, input, unique_count);
        finish_ramp(
            result,
            input,
            repeats,
            unique_count,
            num_points,
            config,
            weights,
        );
        return;
    }

    let mut centred = [0.0f64; MAX_BLOCK * 4];
    let mut centre = [0.0f64; 3];
    let mut axes = [[0.0f64; 3]; 2];
    let axis_count = find_axis_candidates(
        &mut centred,
        &mut centre,
        &normalised,
        repeats,
        unique_count,
        weights,
        &mut axes,
    );
    if axis_count == 0 {
        use_extremes(result, input, unique_count);
        finish_ramp(
            result,
            input,
            repeats,
            unique_count,
            num_points,
            config,
            weights,
        );
        return;
    }

    // rank candidates by the error of their snapped endpoints rather than the search's continuous error, since grid snapping can reorder them
    let mut scratch = RampScratch::new();
    let mut candidate = [0.0f64; 3 * NUM_ENDPOINTS];
    let mut chosen = [0.0f64; 3 * NUM_ENDPOINTS];
    let mut best_error = f64::MAX;
    for axis in &axes[..axis_count] {
        search_direction(
            &mut candidate,
            axis,
            &centred,
            &centre,
            repeats,
            unique_count,
            num_points,
            weights,
            &mut scratch,
        );
        let error = endpoint_error(
            &candidate,
            input,
            repeats,
            unique_count,
            num_points,
            weights,
            &mut scratch,
        );
        if error < best_error {
            best_error = error;
            copy_endpoints(&mut chosen, &candidate);
        }
    }
    copy_endpoints(result, &chosen);
    finish_ramp(
        result,
        input,
        repeats,
        unique_count,
        num_points,
        config,
        weights,
    );
}

fn use_extremes(result: &mut [f64; 3 * NUM_ENDPOINTS], input: &[f64], unique_count: usize) {
    for channel in 0..3 {
        result[channel * 2] = input[channel];
        result[channel * 2 + 1] = input[(unique_count - 1) * 4 + channel];
    }
}

pub(super) fn copy_endpoints(
    target: &mut [f64; 3 * NUM_ENDPOINTS],
    source: &[f64; 3 * NUM_ENDPOINTS],
) {
    target.copy_from_slice(source);
}

pub(super) fn clamp(value: f64) -> f64 {
    value.clamp(0.0, 255.0)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn cluster_error(
    values: &[f64],
    repeats: &[f64],
    ramp: &[f64; 4 * MAX_POINTS],
    colour_count: usize,
    num_points: usize,
    const_ramp: bool,
    weights: &[f64; 3],
) -> f64 {
    let mut error = 0.0;
    let ramp_length = if const_ramp { 1 } else { num_points };
    for index in 0..colour_count {
        let mut shortest = 99_999_999_999.0;
        for point in 0..ramp_length {
            let mut distance = 0.0;
            for channel in 0..3 {
                let delta = values[index * 4 + channel] - ramp[channel * 16 + point];
                distance += delta * delta * weights[channel];
            }
            if distance < shortest {
                shortest = distance;
            }
        }
        error += shortest * repeats[index];
    }
    error
}

#[allow(clippy::too_many_arguments)]
fn cluster_colour_block(
    block: &[i32; MAX_BLOCK],
    endpoints: [u8; 3 * NUM_ENDPOINTS],
    indices: &mut [u8; MAX_BLOCK],
    num_points: usize,
    weights: &[f64; 3],
    use_alpha: bool,
    alpha_threshold: i32,
) -> f64 {
    let low = endpoint_colour(endpoints, 0);
    let high = endpoint_colour(endpoints, 1);
    let (first, second) = if (num_points.is_multiple_of(2) && low <= high)
        || (!num_points.is_multiple_of(2) && low > high)
    {
        (1, 0)
    } else {
        (0, 1)
    };

    let mut ramp_endpoints = [0.0f64; 3 * NUM_ENDPOINTS];
    for channel in 0..3 {
        ramp_endpoints[channel * 2] = f64::from(endpoints[channel * 2 + first]);
        ramp_endpoints[channel * 2 + 1] = f64::from(endpoints[channel * 2 + second]);
    }

    let mut values = [0.0f64; MAX_BLOCK * 4];
    for pixel in 0..MAX_BLOCK {
        values[pixel * 4 + RC] = f64::from((block[pixel] as u32 >> 16) & 0xff);
        values[pixel * 4 + GC] = f64::from((block[pixel] as u32 >> 8) & 0xff);
        values[pixel * 4 + BC] = f64::from(block[pixel] as u32 & 0xff);
        if use_alpha {
            values[pixel * 4 + AC] =
                if ((block[pixel] as u32 >> 24) & 0xff) as i32 >= alpha_threshold {
                    1.0
                } else {
                    0.0
                };
        }
    }

    let mut snapped = [0.0f64; 3 * NUM_ENDPOINTS];
    let eq = make_work_ramp_points(&mut snapped, &ramp_endpoints);
    let mut ramp = [0.0f64; 4 * MAX_POINTS];
    build_ramp(&mut ramp, &snapped, num_points);

    let mut error = 0.0;
    let ramp_length = if eq { 1 } else { num_points };
    for pixel in 0..MAX_BLOCK {
        if use_alpha && values[pixel * 4 + AC] == 0.0 {
            indices[pixel] = num_points as u8;
            continue;
        }
        let mut shortest = 99_999_999_999.0;
        let mut shortest_index = 0;
        for point in 0..ramp_length {
            let mut distance = 0.0;
            for channel in 0..3 {
                let delta = values[pixel * 4 + channel] - ramp[channel * 16 + point];
                distance += delta * delta * weights[channel];
            }
            if distance < shortest {
                shortest = distance;
                shortest_index = point;
            }
        }
        error += shortest;
        indices[pixel] = if shortest_index == num_points - 1 {
            1
        } else if shortest_index != 0 {
            (shortest_index + 1) as u8
        } else {
            0
        };
    }
    error
}

pub(super) fn make_work_ramp_points(
    out: &mut [f64; 3 * NUM_ENDPOINTS],
    input: &[f64; 3 * NUM_ENDPOINTS],
) -> bool {
    let mut eq = true;
    for channel in 0..3 {
        eq = eq && input[channel * 2] == input[channel * 2 + 1];
    }
    for channel in 0..3 {
        let bits = CHANNEL_BIT_WIDTHS[channel];
        let shift = 8 - bits;
        for endpoint in 0..2 {
            let code = (clamp(input[channel * 2 + endpoint]) as i32 as u32) >> shift;
            out[channel * 2 + endpoint] = f64::from((code << shift) | (code >> (2 * bits - 8)));
        }
    }
    eq
}

pub(super) fn build_channel_ramp(
    ramp: &mut [f64; 4 * MAX_POINTS],
    endpoints: &[f64; 3 * NUM_ENDPOINTS],
    num_points: usize,
    channel: usize,
) {
    ramp[channel * 16] = endpoints[channel * 2];
    ramp[channel * 16 + num_points - 1] = endpoints[channel * 2 + 1];
    for point in 1..num_points - 1 {
        let value = (ramp[channel * 16] * (num_points - 1 - point) as f64
            + ramp[channel * 16 + num_points - 1] * point as f64)
            / (num_points - 1) as f64;
        ramp[channel * 16 + point] = value.floor();
    }
}

pub(super) fn build_ramp(
    ramp: &mut [f64; 4 * MAX_POINTS],
    endpoints: &[f64; 3 * NUM_ENDPOINTS],
    num_points: usize,
) {
    for channel in 0..3 {
        build_channel_ramp(ramp, endpoints, num_points, channel);
    }
}

use crate::format::{DxtConfig, Speed};

use super::{
    ALPHA_RANGE, GLOBAL_SEARCH_EXTENT, GLOBAL_SEARCH_STEP, LOCAL_SEARCH_STEP, MAX_BLOCK, MAX_ERROR,
    MAX_POINTS, NUM_ENDPOINTS, REFINE_MOVES,
};

pub fn compress_alpha_block(alphas: &[i32; MAX_BLOCK], config: &DxtConfig, out: &mut [i32; 2]) {
    if config.alpha_speed == Speed::Normal {
        let mut endpoints8 = [0u8; NUM_ENDPOINTS];
        let mut indices8 = [0u8; MAX_BLOCK];
        let mut endpoints6 = [0u8; NUM_ENDPOINTS];
        let mut indices6 = [0u8; MAX_BLOCK];
        let error8 = compress_alpha_ramp(alphas, 8, false, config, &mut endpoints8, &mut indices8);
        let error6 = if error8 == 0.0 {
            f64::MAX
        } else {
            compress_alpha_ramp(alphas, 6, true, config, &mut endpoints6, &mut indices6)
        };
        if error8 <= error6 {
            encode_alpha_block(endpoints8, &indices8, out);
        } else {
            encode_alpha_block(endpoints6, &indices6, out);
        }
    } else {
        let mut endpoints = [0u8; NUM_ENDPOINTS];
        let mut indices = [0u8; MAX_BLOCK];
        compress_alpha_ramp(alphas, 8, false, config, &mut endpoints, &mut indices);
        encode_alpha_block(endpoints, &indices, out);
    }
}

fn compress_alpha_ramp(
    alphas: &[i32; MAX_BLOCK],
    num_points: usize,
    fixed_ramp_points: bool,
    config: &DxtConfig,
    endpoints: &mut [u8; NUM_ENDPOINTS],
    indices: &mut [u8; MAX_BLOCK],
) -> f64 {
    let mut block = [0.0f64; MAX_BLOCK];
    for pixel in 0..MAX_BLOCK {
        block[pixel] = f64::from(alphas[pixel] & 0xff) / 255.0;
    }
    let mut ramp = [0.0f64; NUM_ENDPOINTS];
    let _ = solve_alpha_ramp(&mut ramp, &block, num_points, fixed_ramp_points, config);
    let error = cluster_alpha(indices, &block, &mut ramp, num_points, fixed_ramp_points);
    endpoints[0] = (ramp[0] as i32 & 0xff) as u8;
    endpoints[1] = (ramp[1] as i32 & 0xff) as u8;
    error
}

fn cluster_alpha(
    indices: &mut [u8; MAX_BLOCK],
    block: &[f64; MAX_BLOCK],
    ramp: &mut [f64; NUM_ENDPOINTS],
    num_points: usize,
    fixed_ramp_points: bool,
) -> f64 {
    indices.fill(0);
    if ramp[0] == ramp[1] {
        return 0.0;
    }

    let mut alpha = [0.0f64; MAX_POINTS];
    build_alpha_ramp(&mut alpha, ramp, num_points, fixed_ramp_points);
    let points = if fixed_ramp_points {
        num_points + 2
    } else {
        num_points
    };
    for value in alpha.iter_mut().take(points) {
        *value /= 255.0;
    }

    let mut error = 0.0;
    for pixel in 0..MAX_BLOCK {
        let mut shortest = 10_000_000.0;
        let current = block[pixel];
        for point in 0..points {
            let delta = current - alpha[point];
            let distance = delta * delta;
            if distance < shortest {
                shortest = distance;
                indices[pixel] = point as u8;
            }
        }
        error += shortest;
    }
    error
}

fn build_alpha_ramp(
    ramp_data: &mut [f64; MAX_POINTS],
    ramp: &mut [f64; NUM_ENDPOINTS],
    num_points: usize,
    fixed_ramp_points: bool,
) {
    if ramp[0] == ramp[1] {
        return;
    }
    if (!fixed_ramp_points && ramp[0] <= ramp[1]) || (fixed_ramp_points && ramp[0] > ramp[1]) {
        ramp.swap(0, 1);
    }

    ramp_data[0] = ramp[0];
    ramp_data[1] = ramp[1];
    for point in 1..num_points - 1 {
        ramp_data[point + 1] = ((ramp_data[0] * (num_points - 1 - point) as f64
            + ramp_data[1] * point as f64)
            / (num_points - 1) as f64)
            .floor();
    }
    if fixed_ramp_points {
        ramp_data[num_points] = 0.0;
        ramp_data[num_points + 1] = ALPHA_RANGE - 1.0;
    }
    for value in ramp_data.iter_mut().take(num_points) {
        *value = value.floor();
    }
}

fn solve_alpha_ramp(
    ramp_points: &mut [f64; NUM_ENDPOINTS],
    block: &[f64; MAX_BLOCK],
    num_points: usize,
    fixed_ramp_points: bool,
    config: &DxtConfig,
) -> f64 {
    let mut sorted = *block;
    sorted.sort_by(|left, right| left.partial_cmp(right).expect("finite alpha values"));
    let mut values = [0.0f64; MAX_BLOCK];
    let mut repeats = [0.0f64; MAX_BLOCK];
    let mut unique_count = 0;

    if fixed_ramp_points {
        let mut previous = -2.0;
        for value in &sorted {
            if previous == *value {
                if unique_count > 0 && values[unique_count - 1] == previous {
                    repeats[unique_count - 1] += 1.0;
                }
            } else {
                previous = *value;
                // the 8-point ramp reserves its end codes for alpha 0 and 255 so leave values they already cover out
                if previous > 1.5 / 255.0 && previous < 253.5 / 255.0 {
                    values[unique_count] = *value;
                    repeats[unique_count] = 1.0;
                    unique_count += 1;
                }
            }
        }
    } else {
        let mut previous = -2.0;
        for value in &sorted {
            if previous != *value {
                previous = *value;
                values[unique_count] = previous;
                repeats[unique_count] = 1.0;
                unique_count += 1;
            } else {
                repeats[unique_count - 1] += 1.0;
            }
        }
    }

    let mut ramp = [0.0f64; NUM_ENDPOINTS];
    if unique_count <= 2 {
        match unique_count {
            2 => {
                ramp[0] = (values[0] * (ALPHA_RANGE - 1.0) + 0.5).floor();
                ramp[1] = (values[1] * (ALPHA_RANGE - 1.0) + 0.5).floor();
            }
            1 => {
                ramp[0] = (values[0] * (ALPHA_RANGE - 1.0) + 0.5).floor();
                ramp[1] = ramp[0] + 1.0;
            }
            _ => {
                ramp[0] = 128.0;
                ramp[1] = 129.0;
            }
        }
        apply_alpha_ramp(ramp_points, &mut ramp);
        return 0.0;
    }

    let min_ex = values[0];
    let max_ex = values[unique_count - 1];
    let mut min_r = min_ex;
    let mut max_r = max_ex;
    let mut global_low = 0.0;
    let mut global_high = 0.0;
    let centre = (min_r + max_r) / 2.0;
    let mut global_error = MAX_ERROR;

    if max_ex - min_ex > 48.0 / ALPHA_RANGE {
        let low_lower = (min_r - GLOBAL_SEARCH_EXTENT).max(0.0);
        let high_upper = (max_r + GLOBAL_SEARCH_EXTENT).min(1.0);
        let low_range = centre.min(min_r + GLOBAL_SEARCH_EXTENT);
        let high_range = centre.max(max_r - GLOBAL_SEARCH_EXTENT);
        let mut step_low = low_lower;
        while step_low < low_range {
            let mut step_high = high_upper;
            while high_range <= step_high {
                let error = alpha_search_error(
                    &values[..unique_count],
                    &repeats[..unique_count],
                    global_error,
                    step_low,
                    step_high,
                    unique_count,
                    num_points,
                );
                if error < global_error {
                    global_error = error;
                    global_low = step_low;
                    global_high = step_high;
                }
                step_high -= GLOBAL_SEARCH_STEP;
            }
            step_low += GLOBAL_SEARCH_STEP;
        }
        min_r = global_low;
        max_r = global_high;
    }

    let step = LOCAL_SEARCH_STEP / ALPHA_RANGE;
    let error = refine_alpha_ramp(
        &values,
        &repeats,
        global_error,
        min_r,
        max_r,
        step,
        0.0,
        1.0,
        unique_count,
        num_points,
        config,
    );

    ramp[0] = (min_r * (ALPHA_RANGE - 1.0) + 0.5).floor();
    ramp[1] = (max_r * (ALPHA_RANGE - 1.0) + 0.5).floor();
    apply_alpha_ramp(ramp_points, &mut ramp);
    error
}

fn apply_alpha_ramp(ramp_points: &mut [f64; NUM_ENDPOINTS], ramp: &mut [f64; NUM_ENDPOINTS]) {
    if ramp[0] == ramp[1] {
        if ramp[1] < 255.0 {
            ramp[1] += 1.0;
        } else {
            ramp[1] -= 1.0;
        }
    }
    ramp_points[0] = ramp[0];
    ramp_points[1] = ramp[1];
}

#[allow(clippy::too_many_arguments)]
fn refine_alpha_ramp(
    block: &[f64; MAX_BLOCK],
    repeats: &[f64; MAX_BLOCK],
    max_error_in: f64,
    min_end_in: f64,
    max_end_in: f64,
    step: f64,
    min_bound: f64,
    max_bound: f64,
    count: usize,
    num_points: usize,
    config: &DxtConfig,
) -> f64 {
    let mut max_error = max_error_in;
    let mut min_end = min_end_in;
    let mut max_end = max_end_in;
    loop {
        let mut best_mode = None;
        let mut best_min = min_end;
        let mut best_max = max_end;
        for mode in 0..config.alpha_steps * config.alpha_steps {
            let candidate_min =
                (min_end + step * REFINE_MOVES[mode / config.alpha_steps]).max(min_bound);
            let candidate_max =
                (max_end + step * REFINE_MOVES[mode % config.alpha_steps]).min(max_bound);
            let error = alpha_search_error(
                block,
                repeats,
                max_error,
                candidate_min,
                candidate_max,
                count,
                num_points,
            );
            if error < max_error {
                max_error = error;
                best_mode = Some(mode);
                best_min = candidate_min;
                best_max = candidate_max;
            }
        }
        if best_mode.is_some() {
            min_end = best_min;
            max_end = best_max;
        } else {
            break;
        }
    }
    max_error
}

fn alpha_search_error(
    block: &[f64],
    repeats: &[f64],
    max_error: f64,
    min_end: f64,
    max_end: f64,
    count: usize,
    num_points: usize,
) -> f64 {
    let mut error = 0.0;
    let step = (max_end - min_end) / (num_points - 1) as f64;
    let half_step = step * 0.5;
    let over_step = 1.0 / step;
    for index in 0..count {
        let value = block[index];
        let quantised = if value - min_end <= 0.0 {
            min_end
        } else if value - max_end >= 0.0 {
            max_end
        } else {
            ((value - min_end + half_step) * over_step).floor() * step + min_end
        };
        let delta = value - quantised;
        error += delta * delta * repeats[index];
        if max_error < error {
            error = max_error;
            break;
        }
    }
    error
}

fn encode_alpha_block(
    endpoints: [u8; NUM_ENDPOINTS],
    indices: &[u8; MAX_BLOCK],
    out: &mut [i32; 2],
) {
    let mut low = u32::from(endpoints[0]) | (u32::from(endpoints[1]) << 8);
    let mut high = 0u32;
    for (position, index) in indices.iter().enumerate() {
        let index = u32::from(index & 0x7);
        if position < 5 {
            low |= index << (16 + position * 3);
        } else if position > 5 {
            high |= index << (2 + (position - 6) * 3);
        } else {
            low |= (index & 0x1) << 31;
            high |= (index & 0x6) >> 1;
        }
    }
    out[0] = low as i32;
    out[1] = high as i32;
}

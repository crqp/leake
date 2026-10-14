use crate::format::DxtConfig;

use super::ramp::{
    build_channel_ramp, build_ramp, clamp, cluster_error, copy_endpoints, make_work_ramp_points,
};
use super::{BC, GC, GRID_FACTOR, MAX_BLOCK, MAX_POINTS, NUM_ENDPOINTS, RC, REPLICATE_FACTOR};

#[allow(clippy::too_many_arguments)]
pub(super) fn finish_ramp(
    result: &mut [f64; 3 * NUM_ENDPOINTS],
    input: &[f64],
    repeats: &[f64],
    unique_count: usize,
    num_points: usize,
    config: &DxtConfig,
    weights: &[f64; 3],
) {
    let mut grid_endpoints = [0.0f64; 3 * NUM_ENDPOINTS];
    snap_to_coarse_grid(&mut grid_endpoints, result);
    if config.use_3d_refinement {
        refine3d(
            result,
            &grid_endpoints,
            input,
            repeats,
            unique_count,
            num_points,
            weights,
            config,
        );
    } else {
        refine(
            result,
            &grid_endpoints,
            input,
            repeats,
            unique_count,
            num_points,
            weights,
            config,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn refine(
    out_endpoints: &mut [f64; 3 * NUM_ENDPOINTS],
    input_endpoints: &[f64; 3 * NUM_ENDPOINTS],
    block: &[f64],
    repeats: &[f64],
    colour_count: usize,
    num_points: usize,
    weights: &[f64; 3],
    config: &DxtConfig,
) {
    let mut ramp = [0.0f64; 4 * MAX_POINTS];
    let mut values = [0.0f64; MAX_BLOCK * 4];
    for index in 0..colour_count {
        for channel in 0..3 {
            values[index * 4 + channel] = block[index * 4 + channel];
        }
    }

    let mut current = *input_endpoints;
    let mut best = *input_endpoints;
    let mut work_endpoints = [0.0f64; 3 * NUM_ENDPOINTS];
    let mut error_table = [0.0f64; MAX_POINTS * MAX_BLOCK];
    let mut eq = make_work_ramp_points(&mut work_endpoints, &current);
    build_ramp(&mut ramp, &work_endpoints, num_points);
    let mut best_error = cluster_error(
        &values,
        repeats,
        &ramp,
        colour_count,
        num_points,
        eq,
        weights,
    );
    if best_error == 0.0 || config.rgb_steps == 0 {
        copy_endpoints(out_endpoints, &best);
        return;
    }

    let order = [RC, GC, BC];
    for (step, primary) in order.iter().copied().enumerate() {
        let other0 = order[(step + 1) % 3];
        let other1 = order[(step + 2) % 3];
        for index in 0..colour_count {
            for point in 0..num_points {
                let delta0 = ramp[other0 * 16 + point] - values[index * 4 + other0];
                let delta1 = ramp[other1 * 16 + point] - values[index * 4 + other1];
                error_table[point * MAX_BLOCK + index] =
                    delta0 * delta0 * weights[other0] + delta1 * delta1 * weights[other1];
            }
        }

        let mut best0 = best[primary * 2];
        let mut best1 = best[primary * 2 + 1];
        let steps = config.rgb_steps.min(8) as i32;
        for first_offset in -steps..=steps {
            for second_offset in -steps..=steps {
                current[primary * 2] = clamp(
                    input_endpoints[primary * 2] + f64::from(first_offset) * GRID_FACTOR[primary],
                );
                current[primary * 2 + 1] = clamp(
                    input_endpoints[primary * 2 + 1]
                        + f64::from(second_offset) * GRID_FACTOR[primary],
                );
                eq = make_work_ramp_points(&mut work_endpoints, &current);
                build_channel_ramp(&mut ramp, &work_endpoints, num_points, primary);
                let mut mse = 0.0;
                let ramp_length = if eq { 1 } else { num_points };
                for index in 0..colour_count {
                    let mut min_error: f64 = 10_000_000.0;
                    for point in 0..ramp_length {
                        let delta = ramp[primary * 16 + point] - values[index * 4 + primary];
                        min_error = min_error.min(
                            error_table[point * MAX_BLOCK + index]
                                + delta * delta * weights[primary],
                        );
                    }
                    mse += min_error * repeats[index];
                }
                if mse < best_error {
                    best0 = current[primary * 2];
                    best1 = current[primary * 2 + 1];
                    best_error = mse;
                }
            }
        }
        current[primary * 2] = best0;
        current[primary * 2 + 1] = best1;
        best[primary * 2] = best0;
        best[primary * 2 + 1] = best1;
        if step < 2 {
            make_work_ramp_points(&mut work_endpoints, &current);
            build_ramp(&mut ramp, &work_endpoints, num_points);
        }
    }
    copy_endpoints(out_endpoints, &best);
}

#[allow(clippy::too_many_arguments)]
fn refine3d(
    out_endpoints: &mut [f64; 3 * NUM_ENDPOINTS],
    input_endpoints: &[f64; 3 * NUM_ENDPOINTS],
    block: &[f64],
    repeats: &[f64],
    colour_count: usize,
    num_points: usize,
    weights: &[f64; 3],
    config: &DxtConfig,
) {
    let mut ramp = [0.0f64; 4 * MAX_POINTS];
    let mut values = [0.0f64; MAX_BLOCK * 4];
    for index in 0..colour_count {
        for channel in 0..3 {
            values[index * 4 + channel] = block[index * 4 + channel];
        }
    }

    let base = *input_endpoints;
    let mut current = *input_endpoints;
    copy_endpoints(out_endpoints, input_endpoints);

    let mut work_endpoints = [0.0f64; 3 * NUM_ENDPOINTS];
    let mut eq = make_work_ramp_points(&mut work_endpoints, &current);
    build_ramp(&mut ramp, &work_endpoints, num_points);
    let mut best_error = cluster_error(
        &values,
        repeats,
        &ramp,
        colour_count,
        num_points,
        eq,
        weights,
    );
    if best_error == 0.0 || config.rgb_steps == 0 {
        return;
    }

    let steps = config.rgb_steps.min(8) as i32;
    let mut green_errors = [0.0f64; MAX_POINTS * MAX_BLOCK];
    let mut blue_errors = [0.0f64; MAX_POINTS * MAX_BLOCK];
    for jitter_g0 in -steps..=steps {
        current[GC * 2] = clamp(base[GC * 2] + f64::from(jitter_g0) * GRID_FACTOR[GC]);
        for jitter_g1 in -steps..=steps {
            current[GC * 2 + 1] = clamp(base[GC * 2 + 1] + f64::from(jitter_g1) * GRID_FACTOR[GC]);
            make_work_ramp_points(&mut work_endpoints, &current);
            build_channel_ramp(&mut ramp, &work_endpoints, num_points, GC);
            for index in 0..colour_count {
                for point in 0..num_points {
                    let delta = ramp[GC * 16 + point] - values[index * 4 + GC];
                    green_errors[point * MAX_BLOCK + index] = delta * delta * weights[GC];
                }
            }
            for jitter_b0 in -steps..=steps {
                current[BC * 2] = clamp(base[BC * 2] + f64::from(jitter_b0) * GRID_FACTOR[BC]);
                for jitter_b1 in -steps..=steps {
                    current[BC * 2 + 1] =
                        clamp(base[BC * 2 + 1] + f64::from(jitter_b1) * GRID_FACTOR[BC]);
                    make_work_ramp_points(&mut work_endpoints, &current);
                    build_channel_ramp(&mut ramp, &work_endpoints, num_points, BC);
                    for index in 0..colour_count {
                        for point in 0..num_points {
                            let delta = ramp[BC * 16 + point] - values[index * 4 + BC];
                            blue_errors[point * MAX_BLOCK + index] = green_errors
                                [point * MAX_BLOCK + index]
                                + delta * delta * weights[BC];
                        }
                    }
                    for jitter_r0 in -steps..=steps {
                        current[RC * 2] =
                            clamp(base[RC * 2] + f64::from(jitter_r0) * GRID_FACTOR[RC]);
                        for jitter_r1 in -steps..=steps {
                            current[RC * 2 + 1] =
                                clamp(base[RC * 2 + 1] + f64::from(jitter_r1) * GRID_FACTOR[RC]);
                            eq = make_work_ramp_points(&mut work_endpoints, &current);
                            build_channel_ramp(&mut ramp, &work_endpoints, num_points, RC);
                            let mut mse = 0.0;
                            let ramp_length = if eq { 1 } else { num_points };
                            for index in 0..colour_count {
                                let mut min_error: f64 = 10_000_000.0;
                                for point in 0..ramp_length {
                                    let delta = ramp[RC * 16 + point] - values[index * 4 + RC];
                                    min_error = min_error.min(
                                        blue_errors[point * MAX_BLOCK + index]
                                            + delta * delta * weights[RC],
                                    );
                                }
                                mse += min_error * repeats[index];
                            }
                            if mse < best_error {
                                best_error = mse;
                                copy_endpoints(out_endpoints, &current);
                            }
                        }
                    }
                }
            }
        }
    }
}

fn snap_to_coarse_grid(ramp: &mut [f64; 3 * NUM_ENDPOINTS], min_max: &[f64; 3 * NUM_ENDPOINTS]) {
    for channel in 0..3 {
        for endpoint in 0..2 {
            let mut value = min_max[channel * 2 + endpoint].floor();
            if value <= 0.0 {
                value = 0.0;
            } else {
                value += (128.0 / REPLICATE_FACTOR[channel]).floor()
                    - (value / REPLICATE_FACTOR[channel]).floor();
                value = value.min(255.0);
            }
            ramp[channel * 2 + endpoint] =
                (value / GRID_FACTOR[channel]).floor() * GRID_FACTOR[channel];
        }
    }
}

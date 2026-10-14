use super::ramp::{RampScratch, build_ramp, cluster_error, make_work_ramp_points};
use super::{EPS, EPS2, MAX_ERROR, NUM_ENDPOINTS};

#[allow(clippy::too_many_arguments)]
pub(super) fn search_direction(
    result: &mut [f64; 3 * NUM_ENDPOINTS],
    start_direction: &[f64; 3],
    centred: &[f64],
    centre: &[f64; 3],
    repeats: &[f64],
    unique_count: usize,
    num_points: usize,
    weights: &[f64; 3],
    scratch: &mut RampScratch,
) {
    let mut direction = *start_direction;
    let mut best_error = 10_000_000.0;
    let mut best_direction = direction;
    let mut best_endpoints = [0.0f64; NUM_ENDPOINTS];

    loop {
        scratch.bounds[0] = 1000.0;
        scratch.bounds[1] = -1000.0;
        scratch.endpoints[0] = 0.0;
        scratch.endpoints[1] = 0.0;
        for index in 0..unique_count {
            let value = centred[index * 4] * direction[0]
                + centred[index * 4 + 1] * direction[1]
                + centred[index * 4 + 2] * direction[2];
            scratch.raw_projection[index] = value;
            scratch.projected[index] = value;
            let mut error = 0.0;
            for channel in 0..3 {
                let delta = centred[index * 4 + channel] - direction[channel] * value;
                // weight residuals like the quantiser so the axis search optimises the same metric the final error is measured in
                error += weights[channel] * delta * delta;
            }
            scratch.projection_error[index] = error;
            scratch.bounds[0] = scratch.bounds[0].min(value);
            scratch.bounds[1] = scratch.bounds[1].max(value);
        }

        let scale = [
            scratch.bounds[0] - (scratch.bounds[1] - scratch.bounds[0]) * 0.125,
            scratch.bounds[1] + (scratch.bounds[1] - scratch.bounds[0]) * 0.125,
        ];
        let scale_span = (scale[1] - scale[0]) * (scale[1] - scale[0]);
        let over_span = 1.0 / (scale[1] - scale[0]);
        for index in 0..unique_count {
            scratch.projected[index] = (scratch.projected[index] - scale[0]) * over_span;
            scratch.weighted_repeats[index] = repeats[index] * scale_span;
        }
        for bound in 0..2 {
            scratch.bounds[bound] = (scratch.bounds[bound] - scale[0]) * over_span;
        }

        let mut error = MAX_ERROR;
        let step = 0.025;
        let low_start = if scratch.bounds[0] - 2.0 * step > 0.0 {
            scratch.bounds[0] - 2.0 * step
        } else {
            0.0
        };
        let high_end = if scratch.bounds[1] + 2.0 * step < 1.0 {
            scratch.bounds[1] + 2.0 * step
        } else {
            1.0
        };
        let mut low = low_start;
        for _ in 0..8 {
            let mut high = high_end;
            for _ in 0..8 {
                let candidate = ramp_search_error(
                    &scratch.projected,
                    &scratch.projection_error,
                    &scratch.weighted_repeats,
                    error,
                    low,
                    high,
                    unique_count,
                    num_points,
                );
                if candidate < error {
                    error = candidate;
                    scratch.endpoints[0] = low;
                    scratch.endpoints[1] = high;
                }
                high -= step;
            }
            low += step;
        }
        for endpoint in 0..2 {
            scratch.endpoints[endpoint] =
                scratch.endpoints[endpoint] * (scale[1] - scale[0]) + scale[0];
        }

        if error + 0.001 >= best_error {
            break;
        }

        best_error = error;
        best_direction = direction;
        best_endpoints[0] = scratch.endpoints[0];
        best_endpoints[1] = scratch.endpoints[1];

        let point_step = (scratch.endpoints[1] - scratch.endpoints[0]) / (num_points - 1) as f64;
        let half_step = point_step * 0.5;
        let over_step = 1.0 / point_step;
        let over_last_index = 1.0 / (num_points - 1) as f64;
        let average_index = (num_points - 1) as f64 / 2.0;
        for index in 0..unique_count {
            scratch.ramp_indices[index] =
                if scratch.raw_projection[index] - scratch.endpoints[0] <= 0.0 {
                    0.0
                } else if scratch.raw_projection[index] - scratch.endpoints[1] >= 0.0 {
                    (num_points - 1) as f64
                } else {
                    ((scratch.raw_projection[index] - scratch.endpoints[0] + half_step) * over_step)
                        .floor()
                };
            scratch.ramp_indices[index] =
                (scratch.ramp_indices[index] - average_index) * over_last_index;
        }

        let mut length = 0.0;
        let mut correlation = [0.0f64; 3];
        for index in 0..unique_count {
            let weighted = scratch.ramp_indices[index] * repeats[index];
            length += scratch.ramp_indices[index] * weighted;
            for channel in 0..3 {
                correlation[channel] += centred[index * 4 + channel] * weighted;
            }
        }
        direction = [0.0; 3];
        if length > 0.0 {
            direction[0] = correlation[0] / length;
            direction[1] = correlation[1] / length;
            direction[2] = correlation[2] / length;
            let magnitude = (direction[0] * direction[0]
                + direction[1] * direction[1]
                + direction[2] * direction[2])
                .sqrt();
            for value in &mut direction {
                *value /= magnitude;
            }
        }
    }

    for endpoint in 0..2 {
        for channel in 0..3 {
            result[channel * 2 + endpoint] =
                (best_endpoints[endpoint] * best_direction[channel] + centre[channel]) * 255.0;
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn endpoint_error(
    endpoints: &[f64; 3 * NUM_ENDPOINTS],
    input: &[f64],
    repeats: &[f64],
    unique_count: usize,
    num_points: usize,
    weights: &[f64; 3],
    scratch: &mut RampScratch,
) -> f64 {
    let eq = make_work_ramp_points(&mut scratch.snapped, endpoints);
    build_ramp(&mut scratch.error_ramp, &scratch.snapped, num_points);
    cluster_error(
        input,
        repeats,
        &scratch.error_ramp,
        unique_count,
        num_points,
        eq,
        weights,
    )
}

fn ramp_search_error(
    block: &[f64],
    block_error: &[f64],
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
        error += repeats[index] * delta * delta + block_error[index];
        if max_error < error {
            error = max_error;
            break;
        }
    }
    error
}

#[allow(clippy::too_many_arguments)]
pub(super) fn find_axis_candidates(
    centred: &mut [f64],
    centre: &mut [f64; 3],
    input: &[f64],
    repeats: &[f64],
    colour_count: usize,
    weights: &[f64; 3],
    axes: &mut [[f64; 3]; 2],
) -> usize {
    let mut correlation = [0.0f64; 3];
    let mut spread = [0.0f64; 3];

    let mut point_count = 0.0;
    for index in 0..colour_count {
        for channel in 0..3 {
            centre[channel] += input[index * 4 + channel] * repeats[index];
        }
        point_count += repeats[index];
    }
    for value in centre.iter_mut() {
        *value /= point_count;
    }

    // the heuristic axis is derived from unweighted spread, so also build a covariance weighted like the quantiser and let the caller secondarily score its dominant eigenvector
    let mut covariance = [0.0f64; 9];
    for index in 0..colour_count {
        for channel in 0..3 {
            centred[index * 4 + channel] = input[index * 4 + channel] - centre[channel];
        }
        for channel in 0..3 {
            spread[channel] +=
                centred[index * 4 + channel] * centred[index * 4 + channel] * repeats[index];
            correlation[channel] += centred[index * 4 + channel]
                * centred[index * 4 + (channel + 1) % 3]
                * repeats[index];
            for component in 0..3 {
                covariance[channel * 3 + component] += weights[channel]
                    * centred[index * 4 + channel]
                    * centred[index * 4 + component]
                    * repeats[index];
            }
        }
    }

    let mut strongest = 0;
    let mut largest = 0.0;
    let mut significant = 0;
    let epsilon = point_count * EPS;
    for channel in 0..3 {
        if spread[channel] >= epsilon {
            significant += 1;
        } else {
            spread[channel] = 0.0;
        }
        if largest < spread[channel] {
            largest = spread[channel];
            strongest = channel;
        }
    }

    let epsilon2 = point_count * EPS2;
    if (0..3).all(|channel| spread[channel] < epsilon2) {
        return 0;
    }

    let mut heuristic = [0.0f64; 3];
    match significant {
        1 => heuristic[strongest] = 1.0,
        2 => {
            let second = if spread[(strongest + 1) % 3] > 0.0 {
                (strongest + 1) % 3
            } else {
                (strongest + 2) % 3
            };
            let cross = if second == (strongest + 1) % 3 {
                correlation[strongest]
            } else {
                correlation[(strongest + 2) % 3]
            };
            heuristic[second] = cross / spread[strongest];
            heuristic[strongest] = 1.0;
        }
        _ => {
            let mut max_determinant = 100_000.0;
            for channel in 0..3 {
                let determinant = spread[channel] * spread[(channel + 1) % 3]
                    - correlation[channel] * correlation[channel];
                if max_determinant < determinant {
                    max_determinant = determinant;
                    strongest = channel;
                }
            }
            let mut matrix = [0.0f64; 4];
            let column = [
                correlation[(strongest + 2) % 3],
                correlation[(strongest + 1) % 3],
            ];
            matrix[0] = spread[(strongest + 1) % 3];
            matrix[3] = spread[strongest];
            matrix[1] = -correlation[strongest];
            matrix[2] = matrix[1];
            let first = (matrix[0] * column[0] + matrix[1] * column[1]) / max_determinant;
            let second = (matrix[2] * column[0] + matrix[3] * column[1]) / max_determinant;
            heuristic[strongest] = 1.0;
            heuristic[(strongest + 1) % 3] = 1.0;
            heuristic[(strongest + 2) % 3] = first + second;
        }
    }
    normalise(&mut heuristic);

    let Some(mut eigen) = dominant_eigenvector(&covariance) else {
        axes[0] = heuristic;
        return 1;
    };
    normalise(&mut eigen);
    let dot = heuristic[0] * eigen[0] + heuristic[1] * eigen[1] + heuristic[2] * eigen[2];
    // directions are unsigned, so a near-unit absolute dot means both candidates describe the same line; don't search them
    if dot.abs() < 0.999 {
        axes[0] = heuristic;
        axes[1] = eigen;
        2
    } else {
        axes[0] = heuristic;
        1
    }
}

fn normalise(vector: &mut [f64; 3]) {
    let magnitude = (vector[0] * vector[0] + vector[1] * vector[1] + vector[2] * vector[2]).sqrt();
    if magnitude > 0.0 {
        for value in vector.iter_mut() {
            *value /= magnitude;
        }
    }
}

fn dominant_eigenvector(covariance: &[f64; 9]) -> Option<[f64; 3]> {
    let mut vector = [1.0f64; 3];
    for _ in 0..32 {
        let next = [
            covariance[0] * vector[0] + covariance[1] * vector[1] + covariance[2] * vector[2],
            covariance[3] * vector[0] + covariance[4] * vector[1] + covariance[5] * vector[2],
            covariance[6] * vector[0] + covariance[7] * vector[1] + covariance[8] * vector[2],
        ];
        let length = (next[0] * next[0] + next[1] * next[1] + next[2] * next[2]).sqrt();
        // a collapsed vector means the weighted covariance is degenerate, so report failure and let the caller fall back
        if length <= 1e-12 {
            return None;
        }
        for channel in 0..3 {
            vector[channel] = next[channel] / length;
        }
    }
    Some(vector)
}

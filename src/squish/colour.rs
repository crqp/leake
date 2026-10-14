use crate::format::{DxtConfig, Speed};

use super::ramp::compress_ramp;
use super::{BASE_WEIGHTS, BC, GC, MAX_BLOCK, NUM_ENDPOINTS, RC};

pub fn compress_rgb_block(
    block: &[i32; MAX_BLOCK],
    config: &DxtConfig,
    dxt1: bool,
    alpha_threshold: i32,
    out: &mut [i32; 2],
) {
    let weights = colour_weightings(block, config.adaptive_weighting);
    let mut endpoints3 = [0u8; 3 * NUM_ENDPOINTS];
    let mut indices3 = [0u8; MAX_BLOCK];
    let mut endpoints4 = [0u8; 3 * NUM_ENDPOINTS];
    let mut indices4 = [0u8; MAX_BLOCK];

    if dxt1 && config.colour_speed == Speed::Normal {
        let three = compress_ramp(
            block,
            3,
            config,
            &weights,
            true,
            alpha_threshold,
            &mut endpoints3,
            &mut indices3,
        );
        let four = if three.error == Some(0.0) {
            None
        } else {
            Some(compress_ramp(
                block,
                4,
                config,
                &weights,
                true,
                alpha_threshold,
                &mut endpoints4,
                &mut indices4,
            ))
        };
        if four.as_ref().is_none_or(|ramp| {
            !ramp.usable()
                || three
                    .error
                    .is_some_and(|error| error <= ramp.error.unwrap_or(f64::MAX))
        }) {
            pack_colour_block(endpoints3, &indices3, three.three_point, out);
        } else if let Some(four) = &four {
            pack_colour_block(endpoints4, &indices4, four.three_point, out);
        }
        return;
    }

    let four = compress_ramp(
        block,
        4,
        config,
        &weights,
        dxt1,
        alpha_threshold,
        &mut endpoints4,
        &mut indices4,
    );
    if four.usable() {
        pack_colour_block(endpoints4, &indices4, four.three_point, out);
        return;
    }
    let three = compress_ramp(
        block,
        3,
        config,
        &weights,
        dxt1,
        alpha_threshold,
        &mut endpoints3,
        &mut indices3,
    );
    pack_colour_block(endpoints3, &indices3, three.three_point, out);
}

fn colour_weightings(block: &[i32; MAX_BLOCK], adaptive: bool) -> [f64; 3] {
    let mut weights = BASE_WEIGHTS;
    if !adaptive {
        return weights;
    }

    let mut medians = [0.0f64; 3];
    for value in block {
        medians[BC] += f64::from(*value & 0xff);
        medians[GC] += f64::from((*value as u32 >> 8) & 0xff);
        medians[RC] += f64::from((*value as u32 >> 16) & 0xff);
    }
    for median in &mut medians {
        *median /= MAX_BLOCK as f64;
    }

    let largest = medians[0].max(medians[1]).max(medians[2]);
    if largest > 0.0 {
        for median in &mut medians {
            *median /= largest;
        }
    } else {
        medians.fill(1.0);
    }

    let scale = 1.0 / (BASE_WEIGHTS[0] + BASE_WEIGHTS[1] + BASE_WEIGHTS[2]);
    for channel in 0..3 {
        let base = BASE_WEIGHTS[channel] * scale;
        weights[channel] = (base * 3.0 * medians[channel] + base) * 0.25;
    }
    let normalise = 1.0 / (weights[0] + weights[1] + weights[2]);
    for weight in &mut weights {
        *weight *= normalise;
    }
    weights
}

fn packed_colour(red: u32, green: u32, blue: u32) -> u32 {
    ((red >> 3) << 11) | ((green >> 2) << 5) | (blue >> 3)
}

pub(super) fn endpoint_colour(endpoints: [u8; 3 * NUM_ENDPOINTS], index: usize) -> u32 {
    packed_colour(
        u32::from(endpoints[RC * NUM_ENDPOINTS + index]),
        u32::from(endpoints[GC * NUM_ENDPOINTS + index]),
        u32::from(endpoints[BC * NUM_ENDPOINTS + index]),
    )
}

fn pack_colour_block(
    endpoints: [u8; 3 * NUM_ENDPOINTS],
    indices: &[u8; MAX_BLOCK],
    three_point: bool,
    out: &mut [i32; 2],
) {
    let low = endpoint_colour(endpoints, 0);
    let high = endpoint_colour(endpoints, 1);
    let swap = if three_point { low > high } else { low <= high };
    out[0] = if swap {
        (high | (low << 16)) as i32
    } else {
        (low | (high << 16)) as i32
    };
    let mut packed = 0u32;
    for (position, index) in indices.iter().enumerate() {
        packed |= u32::from(index & 0x3) << (2 * position);
    }
    out[1] = packed as i32;
}

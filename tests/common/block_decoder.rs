#![allow(
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::manual_midpoint,
    clippy::needless_range_loop
)]

pub fn decode_dxt1(
    data: &[u8],
    offset: usize,
    block_count_x: usize,
    width: u32,
    height: u32,
) -> Vec<u32> {
    decode(data, offset, block_count_x, width, height, false)
}

pub fn decode_dxt5(
    data: &[u8],
    offset: usize,
    block_count_x: usize,
    width: u32,
    height: u32,
) -> Vec<u32> {
    decode(data, offset, block_count_x, width, height, true)
}

fn decode(
    data: &[u8],
    offset: usize,
    block_count_x: usize,
    width: u32,
    height: u32,
    alpha_block: bool,
) -> Vec<u32> {
    let mut pixels = vec![0u32; (width * height) as usize];
    let block_bytes = if alpha_block { 16 } else { 8 };
    let block_count_y = height.div_ceil(4) as usize;
    for block_y in 0..block_count_y {
        for block_x in 0..block_count_x {
            let block_offset = offset + (block_y * block_count_x + block_x) * block_bytes;
            let colour = decode_colour(
                data,
                if alpha_block {
                    block_offset + 8
                } else {
                    block_offset
                },
            );
            let alpha = alpha_block.then(|| decode_alpha_block(data, block_offset));
            for y in 0..4 {
                for x in 0..4 {
                    let pixel_x = block_x * 4 + x;
                    let pixel_y = block_y * 4 + y;
                    if pixel_x >= width as usize || pixel_y >= height as usize {
                        continue;
                    }
                    let index = y * 4 + x;
                    let argb = colour[index];
                    pixels[pixel_y * width as usize + pixel_x] = match &alpha {
                        Some(alpha) => (alpha[index] << 24) | (argb & 0x00ff_ffff),
                        None => argb,
                    };
                }
            }
        }
    }
    pixels
}

fn decode_colour(data: &[u8], offset: usize) -> [u32; 16] {
    let c0 = little_endian16(data, offset);
    let c1 = little_endian16(data, offset + 2);
    let indices = little_endian32(data, offset + 4);
    let mut ramp = [[0u32; 3]; 4];
    ramp[0] = expand565(c0);
    ramp[1] = expand565(c1);
    if c0 > c1 {
        for channel in 0..3 {
            ramp[2][channel] = (2 * ramp[0][channel] + ramp[1][channel]) / 3;
            ramp[3][channel] = (ramp[0][channel] + 2 * ramp[1][channel]) / 3;
        }
    } else {
        for channel in 0..3 {
            ramp[2][channel] = (ramp[0][channel] + ramp[1][channel]) / 2;
        }
        ramp[3] = [0, 0, 0];
    }
    let mut decoded = [0u32; 16];
    for (pixel, decoded_pixel) in decoded.iter_mut().enumerate() {
        let index = ((indices >> (2 * pixel)) & 0x3) as usize;
        let colour = ramp[index];
        let alpha = if index == 3 && c0 <= c1 { 0 } else { 255 };
        *decoded_pixel = (alpha << 24) | (colour[0] << 16) | (colour[1] << 8) | colour[2];
    }
    decoded
}

pub fn decode_alpha_block(data: &[u8], offset: usize) -> [u32; 16] {
    let a0 = u32::from(data[offset]);
    let a1 = u32::from(data[offset + 1]);
    let mut ramp = [0u32; 8];
    ramp[0] = a0;
    ramp[1] = a1;
    if a0 > a1 {
        for step in 1..=6u32 {
            ramp[(step + 1) as usize] = ((7 - step) * a0 + step * a1) / 7;
        }
    } else {
        for step in 1..=4u32 {
            ramp[(step + 1) as usize] = ((5 - step) * a0 + step * a1) / 5;
        }
        ramp[6] = 0;
        ramp[7] = 255;
    }
    let mut indices = 0u64;
    for byte in 0..6 {
        indices |= u64::from(data[offset + 2 + byte]) << (8 * byte);
    }
    let mut decoded = [0u32; 16];
    for (pixel, decoded_pixel) in decoded.iter_mut().enumerate() {
        *decoded_pixel = ramp[((indices >> (3 * pixel)) & 0x7) as usize];
    }
    decoded
}

fn expand565(value: u32) -> [u32; 3] {
    let red = (value >> 11) & 0x1f;
    let green = (value >> 5) & 0x3f;
    let blue = value & 0x1f;
    [
        (red << 3) | (red >> 2),
        (green << 2) | (green >> 4),
        (blue << 3) | (blue >> 2),
    ]
}

fn little_endian16(data: &[u8], offset: usize) -> u32 {
    u32::from(data[offset]) | (u32::from(data[offset + 1]) << 8)
}

fn little_endian32(data: &[u8], offset: usize) -> u32 {
    let mut value = 0;
    for byte in 0..4 {
        value |= u32::from(data[offset + byte]) << (8 * byte);
    }
    value
}

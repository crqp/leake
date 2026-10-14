use std::sync::Mutex;

use rayon::prelude::*;

use crate::format::{DxtConfig, Quality, VtfFormat};
use crate::frame::Frame;
use crate::quantise;
use crate::squish;

const ALPHA_THRESHOLD: i32 = 127;

const BLOCK: usize = 4;

#[must_use]
pub fn encode(
    raster: &Frame,
    format: VtfFormat,
    quality: Quality,
    dither: bool,
    jobs: usize,
    on_progress: Option<&mut (dyn FnMut(usize, usize) + Send)>,
) -> Vec<u8> {
    if format.is_dxt() {
        encode_dxt(raster, format, quality.config(), jobs, on_progress)
    } else {
        encode_packed(raster, format, dither, jobs, on_progress)
    }
}

struct ProgressSink<'a> {
    done: usize,
    total: usize,
    callback: &'a mut (dyn FnMut(usize, usize) + Send),
}

fn report(sink: &Mutex<ProgressSink<'_>>, delta: usize) {
    let mut state = sink.lock().expect("progress mutex");
    state.done += delta;
    let (done, total) = (state.done, state.total);
    (state.callback)(done, total);
}

fn encode_dxt(
    raster: &Frame,
    format: VtfFormat,
    config: DxtConfig,
    jobs: usize,
    on_progress: Option<&mut (dyn FnMut(usize, usize) + Send)>,
) -> Vec<u8> {
    let blocks_x =
        usize::try_from(raster.width.div_ceil(BLOCK as u32)).expect("block count fits in memory");
    let blocks_y =
        usize::try_from(raster.height.div_ceil(BLOCK as u32)).expect("block count fits in memory");
    let block_bytes = format.bytes_per_block();
    let row_bytes = blocks_x * block_bytes;
    let mut output = vec![0u8; blocks_y * row_bytes];
    encode_rows(
        &mut output,
        blocks_y,
        row_bytes,
        jobs,
        on_progress,
        |first_row, chunk| {
            encode_dxt_chunk(raster, format, &config, blocks_x, chunk, first_row);
        },
    );
    output
}

fn encode_rows(
    output: &mut [u8],
    row_count: usize,
    row_bytes: usize,
    jobs: usize,
    on_progress: Option<&mut (dyn FnMut(usize, usize) + Send)>,
    encode_chunk: impl Fn(usize, &mut [u8]) + Sync,
) {
    let rows_per_task = (row_count / jobs.saturating_mul(4)).max(1);
    let chunk_bytes = rows_per_task * row_bytes;
    if jobs <= 1 {
        let mut sink = on_progress.map(|callback| ProgressSink {
            done: 0,
            total: row_count,
            callback,
        });
        for (chunk_index, chunk) in output.chunks_mut(chunk_bytes).enumerate() {
            let first_row = chunk_index * rows_per_task;
            let rows = chunk.len() / row_bytes;
            encode_chunk(first_row, chunk);
            if let Some(sink) = &mut sink {
                sink.done += rows;
                (sink.callback)(sink.done, sink.total);
            }
        }
        return;
    }

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(jobs)
        .build()
        .expect("encoder thread pool");
    let sink = on_progress.map(|callback| {
        Mutex::new(ProgressSink {
            done: 0,
            total: row_count,
            callback,
        })
    });
    pool.install(|| {
        output
            .par_chunks_mut(chunk_bytes)
            .enumerate()
            .for_each(|(chunk_index, chunk)| {
                let first_row = chunk_index * rows_per_task;
                let rows = chunk.len() / row_bytes;
                encode_chunk(first_row, chunk);
                if let Some(sink) = &sink {
                    report(sink, rows);
                }
            });
    });
}

fn encode_dxt_chunk(
    raster: &Frame,
    format: VtfFormat,
    config: &DxtConfig,
    blocks_x: usize,
    chunk: &mut [u8],
    first_row: usize,
) {
    let block_bytes = format.bytes_per_block();
    let rows = chunk.len() / (blocks_x * block_bytes);
    let mut block = [0i32; BLOCK * BLOCK];
    let mut alphas = [0i32; BLOCK * BLOCK];
    let mut colour = [0i32; 2];
    let mut alpha = [0i32; 2];
    for row in 0..rows {
        let block_row = first_row + row;
        for column in 0..blocks_x {
            read_block(raster, column, block_row, &mut block, &mut alphas);
            let mut offset = row * blocks_x * block_bytes + column * block_bytes;
            if format == VtfFormat::Dxt5 {
                squish::compress_alpha_block(&alphas, config, &mut alpha);
                write_dwords(chunk, offset, &alpha);
                offset += 8;
            }
            squish::compress_rgb_block(
                &block,
                config,
                format == VtfFormat::Dxt1,
                ALPHA_THRESHOLD,
                &mut colour,
            );
            write_dwords(chunk, offset, &colour);
        }
    }
}

fn read_block(
    raster: &Frame,
    block_column: usize,
    block_row: usize,
    block: &mut [i32; BLOCK * BLOCK],
    alphas: &mut [i32; BLOCK * BLOCK],
) {
    let mut index = 0;
    for y in 0..BLOCK {
        let pixel_y = block_row * BLOCK + y;
        for x in 0..BLOCK {
            let pixel_x = block_column * BLOCK + x;
            if pixel_x >= raster.width as usize || pixel_y >= raster.height as usize {
                block[index] = 0;
                alphas[index] = 0;
            } else {
                let base = (pixel_y * raster.width as usize + pixel_x) * 4;
                let red = i32::from(raster.rgba[base]);
                let green = i32::from(raster.rgba[base + 1]);
                let blue = i32::from(raster.rgba[base + 2]);
                let alpha = i32::from(raster.rgba[base + 3]);
                block[index] = ((u32::from(alpha as u8) << 24)
                    | (u32::from(red as u8) << 16)
                    | (u32::from(green as u8) << 8)
                    | u32::from(blue as u8)) as i32;
                alphas[index] = alpha;
            }
            index += 1;
        }
    }
}

fn encode_packed(
    raster: &Frame,
    format: VtfFormat,
    dither: bool,
    jobs: usize,
    on_progress: Option<&mut (dyn FnMut(usize, usize) + Send)>,
) -> Vec<u8> {
    let bytes_per_pixel = bytes_per_pixel(format);
    let height = raster.height as usize;
    let width = raster.width as usize;
    let row_bytes = width * bytes_per_pixel;
    let mut output = vec![0u8; height * row_bytes];
    encode_rows(
        &mut output,
        height,
        row_bytes,
        jobs,
        on_progress,
        |first_row, chunk| {
            encode_packed_chunk(raster, format, dither, chunk, first_row);
        },
    );
    output
}

fn encode_packed_chunk(
    raster: &Frame,
    format: VtfFormat,
    dither: bool,
    chunk: &mut [u8],
    first_row: usize,
) {
    let bytes_per_pixel = bytes_per_pixel(format);
    let width = raster.width as usize;
    let rows = chunk.len() / (width * bytes_per_pixel);
    let mut reducer = quantise::reducer(format, dither);
    for row in 0..rows {
        let y = first_row + row;
        let mut offset = row * width * bytes_per_pixel;
        for x in 0..width {
            let base = (y * width + x) * 4;
            let pixel = (u32::from(raster.rgba[base + 3]) << 24)
                | (u32::from(raster.rgba[base]) << 16)
                | (u32::from(raster.rgba[base + 1]) << 8)
                | u32::from(raster.rgba[base + 2]);
            write_pixel(
                reducer.reduce(pixel, x as u32, y as u32),
                format,
                chunk,
                offset,
            );
            offset += bytes_per_pixel;
        }
    }
}

fn write_pixel(pixel: u32, format: VtfFormat, target: &mut [u8], offset: usize) {
    let red = (pixel >> 16) & 0xff;
    let green = (pixel >> 8) & 0xff;
    let blue = pixel & 0xff;
    let alpha = (pixel >> 24) & 0xff;
    match format {
        VtfFormat::Rgba8888 => {
            target[offset] = red as u8;
            target[offset + 1] = green as u8;
            target[offset + 2] = blue as u8;
            target[offset + 3] = alpha as u8;
        }
        VtfFormat::Rgb888 => {
            target[offset] = red as u8;
            target[offset + 1] = green as u8;
            target[offset + 2] = blue as u8;
        }
        VtfFormat::Rgb565 => write_short(
            target,
            offset,
            (red >> 3) | ((green >> 2) << 5) | ((blue >> 3) << 11),
        ),
        VtfFormat::Bgra5551 => write_short(
            target,
            offset,
            (blue >> 3) | ((green >> 3) << 5) | ((red >> 3) << 10) | ((alpha >> 7) << 15),
        ),
        VtfFormat::Bgra4444 => write_short(
            target,
            offset,
            (blue >> 4) | ((green >> 4) << 4) | ((red >> 4) << 8) | ((alpha >> 4) << 12),
        ),
        VtfFormat::Dxt1 | VtfFormat::Dxt5 => unreachable!("DXT formats are not packed"),
    }
}

fn bytes_per_pixel(format: VtfFormat) -> usize {
    match format {
        VtfFormat::Rgba8888 => 4,
        VtfFormat::Rgb888 => 3,
        _ => 2,
    }
}

fn write_short(target: &mut [u8], offset: usize, value: u32) {
    target[offset] = (value & 0xff) as u8;
    target[offset + 1] = ((value >> 8) & 0xff) as u8;
}

fn write_dwords(target: &mut [u8], offset: usize, dwords: &[i32]) {
    let mut position = offset;
    for dword in dwords {
        target[position..position + 4].copy_from_slice(&dword.to_le_bytes());
        position += 4;
    }
}

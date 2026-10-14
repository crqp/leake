use std::collections::HashMap;

use fast_image_resize::{
    Filter, FilterType, IntoImageView, ResizeAlg, ResizeOptions, Resizer, images::Image,
};
use image::imageops::overlay;

use crate::budget::Layout;
use crate::error::{ImportError, Result};
use crate::frame::Frame;

pub fn slice_strip(image: &Frame, frame_height: u32) -> Result<Vec<Frame>> {
    let frames = image.height / frame_height;
    if frames < 1 {
        return Err(ImportError::new(format!(
            "the input is {}px tall, which is less than one frame of {frame_height}px",
            image.height
        )));
    }
    let mut sliced = Vec::with_capacity(frames as usize);
    for index in 0..frames {
        let start = index as usize * frame_height as usize * image.width as usize * 4;
        let end = (index + 1) as usize * frame_height as usize * image.width as usize * 4;
        sliced.push(Frame::new(
            image.width,
            frame_height,
            image.rgba[start..end].to_vec(),
        ));
    }
    Ok(sliced)
}

#[must_use]
pub fn build<S: std::hash::BuildHasher>(
    frames: &[Frame],
    layout: &Layout,
    rescale: bool,
    custom_levels: &HashMap<u32, Vec<Frame>, S>,
) -> Vec<Frame> {
    let mut levels = Vec::with_capacity(layout.mip_count as usize + 1);
    levels.push(atlas(frames, layout.width, layout.height, rescale));
    for level in 1..=layout.mip_count {
        let level_width = layout.width >> level;
        let level_height = (layout.height >> level) * layout.frames;
        let next = match custom_levels.get(&level) {
            Some(custom) => atlas(
                &cycle(custom, layout.frames),
                level_width,
                level_height / layout.frames,
                rescale,
            ),
            None => resample(&levels[level as usize - 1], level_width, level_height),
        };
        levels.push(next);
    }
    levels
}

// a custom mip level may supply fewer images than the texture has frames, so repeat them
fn cycle(frames: &[Frame], count: u32) -> Vec<Frame> {
    (0..count)
        .map(|index| frames[index as usize % frames.len()].clone())
        .collect()
}

fn atlas(frames: &[Frame], width: u32, cell_height: u32, rescale: bool) -> Frame {
    let mut canvas = image::RgbaImage::new(width, cell_height * frames.len() as u32);
    for (index, frame) in frames.iter().enumerate() {
        let scale = if rescale {
            (f64::from(cell_height) / f64::from(frame.height))
                .min(f64::from(width) / f64::from(frame.width))
        } else {
            1.0
        };
        let draw_width = ((f64::from(frame.width) * scale).round() as u32).max(1);
        let draw_height = ((f64::from(frame.height) * scale).round() as u32).max(1);
        let x = (i64::from(width) - i64::from(draw_width)) / 2;
        let y = index as i64 * i64::from(cell_height)
            + (i64::from(cell_height) - i64::from(draw_height)) / 2;
        let source = frame.to_image();
        let drawn = if draw_width == frame.width && draw_height == frame.height {
            source
        } else {
            resize(&source, draw_width, draw_height)
        };
        overlay(&mut canvas, &drawn, x, y);
    }
    Frame::from_image(canvas)
}

fn resample(source: &Frame, width: u32, height: u32) -> Frame {
    Frame::from_image(resize(&source.to_image(), width, height))
}

fn resize(source: &image::RgbaImage, width: u32, height: u32) -> image::RgbaImage {
    let filter =
        FilterType::Custom(Filter::new("MKS2021", mks2021, 4.5).expect("valid MKS2021 filter"));
    let options = ResizeOptions::new().resize_alg(ResizeAlg::Convolution(filter));
    let pixel_type = source.pixel_type().expect("RGBA image has a pixel type");
    let mut destination = Image::new(width, height, pixel_type);
    Resizer::new()
        .resize(source, &mut destination, &options)
        .expect("valid resize dimensions and pixel type");
    image::RgbaImage::from_raw(width, height, destination.buffer().to_vec())
        .expect("valid resized image buffer")
}

fn mks2021(value: f64) -> f64 {
    let x = value.abs();
    if x < 0.5 {
        577.0 / 576.0 - 239.0 / 144.0 * x * x
    } else if x < 1.5 {
        35.0 / 36.0 * (x - 1.0) * (x - 239.0 / 140.0)
    } else if x < 2.5 {
        1.0 / 6.0 * (x - 2.0) * (65.0 / 24.0 - x)
    } else if x < 3.5 {
        1.0 / 36.0 * (x - 3.0) * (x - 3.75)
    } else if x < 4.5 {
        -1.0 / 288.0 * (x - 4.5) * (x - 4.5)
    } else {
        0.0
    }
}

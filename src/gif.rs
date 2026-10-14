use std::io::Cursor;

use image::AnimationDecoder;
use image::codecs::gif::GifDecoder;

use crate::budget::FrameBudget;
use crate::error::{ImportError, Result};
use crate::frame::Frame;
use crate::options::SprayOptions;

// GIFs usually omit GCE or declare a zero delay so apply a floor to keep them on screen
const MIN_DELAY: u32 = 10;

#[must_use]
pub fn is_animated(bytes: &[u8]) -> bool {
    let Ok(decoder) = GifDecoder::new(Cursor::new(bytes)) else {
        return false;
    };
    let mut frames = decoder.into_frames();
    frames.next().is_some() && frames.next().is_some()
}

pub fn read(
    bytes: &[u8],
    options: &SprayOptions,
    mut budget: Option<&mut FrameBudget>,
) -> Result<Vec<Frame>> {
    let gif_decoder = GifDecoder::new(Cursor::new(bytes))
        .map_err(|error| ImportError::new(format!("cannot decode the GIF. {error}")))?;
    let composited = gif_decoder
        .into_frames()
        .collect_frames()
        .map_err(|error| ImportError::new(format!("cannot decode the GIF. {error}")))?;

    let frame_count = composited.len() as i32;
    let start = (options.start as i32).clamp(0, frame_count);
    let end = (options.end.map_or(frame_count, |value| value as i32)).clamp(start, frame_count);
    // GIF frame delays are counted in hundredths of a second and one spray slot lasts 20 of them
    let slot = options.speed * 20.0;
    let mut frames = Vec::new();
    let mut elapsed = 0.0;

    for (index, composited_frame) in composited.into_iter().enumerate().take(end as usize) {
        let (numer, denom) = composited_frame.delay().numer_denom_ms();
        let delay = (numer / (denom * 10)).max(MIN_DELAY);
        if index as i32 >= start {
            // repeat a frame once for every spray slot it spans so slow frames hold the screen for longer
            let repeats = if options.all_frames {
                1
            } else {
                ((elapsed + f64::from(delay)) / slot).ceil() as i32 - (elapsed / slot).ceil() as i32
            };
            if repeats > 0 {
                let frame = Frame::from_image(composited_frame.into_buffer());
                for _ in 0..repeats {
                    if budget.as_deref_mut().is_none_or(|budget| !budget.stop()) {
                        frames.push(frame.clone());
                    }
                }
                if budget.as_deref().is_some_and(|budget| budget.truncated) {
                    return Ok(frames);
                }
            }
        }
        elapsed += f64::from(delay);
    }
    Ok(frames)
}

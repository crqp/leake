use std::path::{Path, PathBuf};

use image::ImageFormat;

use crate::budget::FrameBudget;
use crate::error::{ImportError, Result};
use crate::frame::Frame;
use crate::gif;
use crate::options::SprayOptions;
use crate::video;

const VIDEO_EXTENSIONS: [&str; 13] = [
    "mp4", "webm", "mkv", "avi", "mov", "m4v", "ogv", "wmv", "flv", "ts", "mpg", "mpeg", "3gp",
];

pub enum DecodedInput {
    Frames(Vec<Frame>),
    Strip(Frame),
}

pub fn read(options: &SprayOptions, budget: Option<&mut FrameBudget>) -> Result<DecodedInput> {
    let first = options
        .inputs
        .first()
        .ok_or_else(|| ImportError::new("no input was given"))?;
    let bytes = read_bytes(first)?;
    if detect_format(&bytes, first).is_none() {
        if is_video(first) {
            return Ok(DecodedInput::Frames(video::decode(first, options, budget)?));
        }
        return Err(ImportError::new(format!(
            "cannot decode {}. The format is not one of the supported image or video types",
            file_name(first)
        )));
    }
    if gif::is_animated(&bytes) {
        return Ok(DecodedInput::Frames(gif::read(&bytes, options, budget)?));
    }

    let frames = decode_stills(&options.inputs)?;
    if options.strip {
        if frames.len() != 1 {
            return Err(ImportError::new(format!(
                "--strip takes a single tall image, but {} images were given",
                frames.len()
            )));
        }
        let image = frames.into_iter().next().expect("one frame");
        return Ok(DecodedInput::Strip(image));
    }
    Ok(DecodedInput::Frames(frames))
}

pub(crate) fn decode_image(bytes: &[u8], path: &Path) -> Result<Frame> {
    let format = detect_format(bytes, path)
        .ok_or_else(|| ImportError::new("no decoder accepted this file"))?;
    let image = image::load_from_memory_with_format(bytes, format)
        .map_err(|error| ImportError::new(error.to_string()))?;
    Ok(Frame::from_image(image.into_rgba8()))
}

pub(crate) fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

fn read_bytes(path: &Path) -> Result<Vec<u8>> {
    std::fs::read(path)
        .map_err(|error| ImportError::new(format!("cannot read {}. {error}", file_name(path))))
}

fn detect_format(bytes: &[u8], path: &Path) -> Option<ImageFormat> {
    image::guess_format(bytes)
        .ok()
        .or_else(|| ImageFormat::from_path(path).ok())
}

fn is_video(path: &Path) -> bool {
    path.extension()
        .and_then(std::ffi::OsStr::to_str)
        .is_some_and(|extension| {
            VIDEO_EXTENSIONS
                .iter()
                .any(|candidate| extension.eq_ignore_ascii_case(candidate))
        })
}

fn decode_stills(paths: &[PathBuf]) -> Result<Vec<Frame>> {
    let mut frames = Vec::with_capacity(paths.len());
    let mut errors = Vec::new();
    for path in paths {
        let decoded = read_bytes(path).and_then(|bytes| decode_image(&bytes, path));
        match decoded {
            Ok(frame) => frames.push(frame),
            Err(error) => errors.push(format!("cannot decode {}. {error}", file_name(path))),
        }
    }
    if errors.is_empty() {
        Ok(frames)
    } else {
        Err(ImportError::new(errors.join("\n")))
    }
}

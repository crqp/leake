use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use crate::budget::FrameBudget;
use crate::error::{ImportError, Result};
use crate::frame::Frame;
use crate::options::SprayOptions;

const SPRAY_FPS: f64 = 5.0;

const SECONDS_PER_SLOT: f64 = 1.0 / SPRAY_FPS;

const MAX_SOURCE_EDGE: u32 = 8192;

// ffmpeg can exit while its error pipe still holds buffered text so wait a bit before reporting
const DRAIN_GRACE: Duration = Duration::from_millis(500);

const PNG_SIGNATURE: [u8; 8] = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];

// a long clip multiplies the atlas height so shrink the decode size as the expected frame count grows
fn max_resolution(frames: i32) -> u32 {
    match frames {
        8193.. => 16,
        2049..=8192 => 32,
        513..=2048 => 64,
        129..=512 => 128,
        33..=128 => 256,
        9..=32 => 512,
        3..=8 => 1024,
        _ => MAX_SOURCE_EDGE,
    }
}

pub fn decode(
    path: &Path,
    options: &SprayOptions,
    budget: Option<&mut FrameBudget>,
) -> Result<Vec<Frame>> {
    let probe = probe(path)?;
    let start = options.start.max(0.0);
    let end = match (options.end, probe.duration) {
        (Some(end), Some(duration)) => Some(end.min(duration)),
        (Some(end), None) => Some(end),
        (None, _) => probe.duration,
    };
    if let Some(end) = end
        && end <= start
    {
        return Err(ImportError::new(format!(
            "the requested clip range is empty. --start is {start} and --end is {end}"
        )));
    }
    let span = end.map(|end| end - start);

    let seconds_per_frame = SECONDS_PER_SLOT * options.speed;
    let expected_frames = span.map(|span| ((span / seconds_per_frame) as i32).max(1));
    let limit = expected_frames.map_or(MAX_SOURCE_EDGE, max_resolution);
    let border = probe.width.max(probe.height);
    let scale = if border > limit {
        f64::from(limit) / f64::from(border)
    } else {
        1.0
    };
    let width = ((f64::from(probe.width) * scale) as u32).max(2);
    let height = ((f64::from(probe.height) * scale) as u32).max(2);

    let mut process = start_ffmpeg(path, start, span, width, height, options.speed)?;
    let stdout = process.stdout.take().expect("ffmpeg stdout");
    let stderr = process.stderr.take().expect("ffmpeg stderr");
    let (complaints_tx, complaints_rx) = mpsc::channel();
    let drain = thread::spawn(move || {
        let mut complaints = String::new();
        for line in BufReader::new(stderr)
            .lines()
            .map_while(std::result::Result::ok)
        {
            complaints.push_str(&line);
            complaints.push('\n');
        }
        let _ = complaints_tx.send(complaints);
    });

    let mut frames = Vec::new();
    let outcome = read_frames(stdout, budget, &mut frames);
    let _ = process.wait();
    let _ = process.kill();
    if outcome.is_ok() && frames.is_empty() {
        let complaints = complaints_rx.recv_timeout(DRAIN_GRACE).unwrap_or_default();
        drop(drain);
        return Err(ImportError::new(frame_failure(start, end, &complaints)));
    }
    drop(drain);
    outcome?;
    Ok(frames)
}

fn read_frames(
    stdout: impl Read,
    mut budget: Option<&mut FrameBudget>,
    frames: &mut Vec<Frame>,
) -> Result<()> {
    let mut reader = BufReader::new(stdout);
    while let Some(png) = read_png(&mut reader)
        .map_err(|error| ImportError::new(format!("cannot read ffmpeg's output. {error}")))?
    {
        // the cap is charged before decoding so the frame over it is dropped undecoded
        if budget.as_deref_mut().is_some_and(FrameBudget::stop) {
            break;
        }
        let image = image::load_from_memory_with_format(&png, image::ImageFormat::Png)
            .map_err(|_| ImportError::new("ffmpeg produced a frame that could not be decoded"))?;
        frames.push(Frame::from_image(image.into_rgba8()));
    }
    Ok(())
}

fn frame_failure(start: f64, end: Option<f64>, complaints: &str) -> String {
    let reason = complaints.lines().find(|line| !line.trim().is_empty());
    let range = end.map_or_else(
        || format!("after {start} seconds"),
        |end| format!("between {start} and {end} seconds"),
    );
    reason.map_or_else(
        || format!("no video frames were found {range}"),
        |reason| format!("no video frames were found {range}. ffmpeg said: {reason}"),
    )
}

struct ProbeResult {
    width: u32,
    height: u32,
    duration: Option<f64>,
}

fn probe(path: &Path) -> Result<ProbeResult> {
    let output = run(Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height:format=duration",
            "-of",
            "default=noprint_wrappers=1",
        ])
        .arg(path))?;
    let mut width = None;
    let mut height = None;
    let mut duration = None;
    for line in output.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key.trim() {
            "width" => width = value.trim().parse().ok(),
            "height" => height = value.trim().parse().ok(),
            "duration" => duration = value.trim().parse().ok(),
            _ => {}
        }
    }
    let (Some(width), Some(height)) = (width, height) else {
        return Err(ImportError::new(format!(
            "{} has no video stream",
            path.display()
        )));
    };
    if width == 0 || height == 0 {
        return Err(ImportError::new(format!(
            "{} has no video stream",
            path.display()
        )));
    }
    Ok(ProbeResult {
        width,
        height,
        duration,
    })
}

fn start_ffmpeg(
    path: &Path,
    start: f64,
    span: Option<f64>,
    width: u32,
    height: u32,
    speed: f64,
) -> Result<Child> {
    let filters = format!("scale={width}:{height},fps={}", SPRAY_FPS / speed);
    let mut command = Command::new("ffmpeg");
    command.args(["-v", "error", "-ss"]).arg(start.to_string());
    if let Some(span) = span {
        command.args(["-t"]).arg(span.to_string());
    }
    command
        .args(["-i"])
        .arg(path)
        .args([
            "-vf",
            &filters,
            "-an",
            "-sn",
            "-dn",
            "-f",
            "image2pipe",
            "-vcodec",
            "png",
            "-",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command.spawn().map_err(|_| {
        ImportError::new(
            "ffmpeg is needed to import video, but it could not be run. Install ffmpeg and try again",
        )
    })
}

fn run(command: &mut Command) -> Result<String> {
    let program = command.get_program().to_string_lossy().into_owned();
    let output = command
        .stdin(Stdio::null())
        .output()
        .map_err(|_| {
            ImportError::new(format!(
                "{program} is needed to import video, but it could not be run. Install ffmpeg and try again"
            ))
        })?;
    if !output.status.success() {
        let errors = String::from_utf8_lossy(&output.stderr);
        let reason = errors
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .unwrap_or("unknown error");
        return Err(ImportError::new(format!(
            "{program} failed on the input: {reason}"
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn read_png(reader: &mut impl Read) -> std::io::Result<Option<Vec<u8>>> {
    let mut signature = [0u8; PNG_SIGNATURE.len()];
    if !read_fully(reader, &mut signature)? || signature != PNG_SIGNATURE {
        return Ok(None);
    }
    let mut image = Vec::new();
    image.extend_from_slice(&signature);
    // image2pipe joins PNGs without a length prefix, so IEND marks each frame boundary
    loop {
        let mut header = [0u8; 8];
        if !read_fully(reader, &mut header)? {
            return Ok(None);
        }
        image.extend_from_slice(&header);
        let length = u32::from_be_bytes([header[0], header[1], header[2], header[3]]) as usize;
        let mut chunk = vec![0u8; length + 4];
        if !read_fully(reader, &mut chunk)? {
            return Ok(None);
        }
        image.extend_from_slice(&chunk);
        if &header[4..8] == b"IEND" {
            return Ok(Some(image));
        }
    }
}

fn read_fully(reader: &mut impl Read, target: &mut [u8]) -> std::io::Result<bool> {
    let mut read = 0;
    while read < target.len() {
        let count = reader.read(&mut target[read..])?;
        if count == 0 {
            return Ok(false);
        }
        read += count;
    }
    Ok(true)
}

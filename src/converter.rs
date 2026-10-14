use std::collections::HashMap;
use std::io::IsTerminal;
use std::path::PathBuf;

use crate::budget::{self, FrameBudget, Layout, LayoutRequest};
use crate::compositor;
use crate::encode;
use crate::error::{ImportError, Result};
use crate::frame::Frame;
use crate::input::{self, DecodedInput};
use crate::options::SprayOptions;
use crate::progress::Progress;
use crate::vtf;

// frame count and height depend on each other, so iterate the layout to a bounded fixed point
const STRIP_ITERATIONS: u32 = 8;

const INVALID_NAME_CHARACTERS: [char; 9] = ['\\', '/', ':', '*', '?', '"', '<', '>', '|'];

pub fn run(options: &SprayOptions) -> Result<()> {
    validate(options)?;

    let requested_width = options.width.or(options.height);
    let requested_height = options.height.or(options.width);
    let mut budget = match (requested_width, requested_height) {
        (Some(width), Some(height)) => Some(FrameBudget::new(width, height, options.format)),
        _ => None,
    };

    let decoded = input::read(options, budget.as_mut())?;
    if budget.as_ref().is_some_and(|budget| budget.truncated) {
        warn(
            "the spray would exceed the size limit, so the rest of the source frames were skipped",
        );
    }

    let (frames, layout) = settle_frames(decoded, options)?;
    if layout.mipmaps && !layout.mipmaps_permitted {
        warn(
            "mipmaps are of no use for this texture, but --mipmaps was given, so they are written anyway",
        );
    }

    if options.dry_run {
        let estimate = budget::est_bytes(
            layout.width,
            layout.height,
            layout.frames,
            options.format,
            layout.mipmaps,
        );
        report_layout(options, &layout, estimate as u64);
        return Ok(());
    }

    let custom_levels = load_custom_levels(options, &layout)?;
    let levels = compositor::build(&frames, &layout, options.rescale, &custom_levels);
    let encoded = encode_levels(&levels, options);
    let actual_bytes = encoded.iter().map(Vec::len).sum::<usize>() + vtf::HEADER_SIZE;
    if actual_bytes as f64 > budget::LIMIT_KIB * 1024.0 {
        warn(&format!(
            "the texture is {}, above the {} Source will load. Reduce the size, or drop the mipmap levels",
            describe(actual_bytes as u64),
            describe((budget::LIMIT_KIB * 1024.0) as u64),
        ));
    }

    let written = write(options, &layout, &encoded)?;
    report_result(options, &layout, actual_bytes as u64, &written);
    Ok(())
}

fn settle_frames(decoded: DecodedInput, options: &SprayOptions) -> Result<(Vec<Frame>, Layout)> {
    match decoded {
        DecodedInput::Frames(frames) => {
            if frames.is_empty() {
                return Err(ImportError::new("no frames were decoded from the input"));
            }
            let source = &frames[0];
            let layout = layout_for(source.width, source.height, frames.len() as u32, options)?;
            Ok((frames, layout))
        }
        DecodedInput::Strip(image) => {
            let mut height = layout_for(image.width, image.height, 1, options)?.height;
            if options.width.is_none() && options.height.is_none() {
                let mut iterations = 0;
                while iterations < STRIP_ITERATIONS {
                    let frames = (image.height / height).max(1);
                    let next = layout_for(image.width, image.height, frames, options)?.height;
                    if next == height {
                        break;
                    }
                    height = next;
                    iterations += 1;
                }
            }
            let frames = compositor::slice_strip(&image, height)?;
            let layout = layout_for(image.width, image.height, frames.len() as u32, options)?;
            Ok((frames, layout))
        }
    }
}

fn layout_for(
    source_width: u32,
    source_height: u32,
    frames: u32,
    options: &SprayOptions,
) -> Result<Layout> {
    budget::resolve(&LayoutRequest {
        source_width,
        source_height,
        frames,
        format: options.format,
        sampling: options.sampling,
        requested_width: options.width,
        requested_height: options.height,
        mipmaps_requested: options.mipmaps,
    })
}

fn load_custom_levels(options: &SprayOptions, layout: &Layout) -> Result<HashMap<u32, Vec<Frame>>> {
    if options.mip_specs.is_empty() {
        return Ok(HashMap::new());
    }
    if !layout.mipmaps {
        warn("--mip images were given, but this texture has no mipmap levels, so they are ignored");
        return Ok(HashMap::new());
    }
    let mut levels = HashMap::new();
    for spec in &options.mip_specs {
        if spec.level < 1 || spec.level > layout.mip_count {
            warn(&format!(
                "--mip {} is outside the {} mipmap levels of this texture, so it is ignored",
                spec.level, layout.mip_count
            ));
            continue;
        }
        let mut frames = Vec::with_capacity(spec.paths.len());
        for path in &spec.paths {
            let bytes = std::fs::read(path).map_err(|error| {
                ImportError::new(format!("cannot decode {}. {error}", input::file_name(path)))
            })?;
            let frame = input::decode_image(&bytes, path).map_err(|error| {
                ImportError::new(format!("cannot decode {}. {error}", input::file_name(path)))
            })?;
            frames.push(frame);
        }
        levels.insert(spec.level, frames);
    }
    Ok(levels)
}

fn encode_levels(levels: &[Frame], options: &SprayOptions) -> Vec<Vec<u8>> {
    let show_progress = options
        .progress
        .unwrap_or_else(|| std::io::stderr().is_terminal());
    let mut progress = show_progress.then(|| Progress::new("encoding"));
    let level_count = levels.len();
    let mut encoded = Vec::with_capacity(level_count);
    for (index, level) in levels.iter().enumerate() {
        let mut sink = |done: usize, total: usize| {
            if let Some(progress) = &mut progress {
                progress.update((index as f64 + done as f64 / total as f64) / level_count as f64);
            }
        };
        encoded.push(encode::encode(
            level,
            options.format,
            options.quality,
            options.dither,
            options.jobs,
            Some(&mut sink),
        ));
    }
    if let Some(progress) = &mut progress {
        progress.close();
    }
    encoded
}

fn write(options: &SprayOptions, layout: &Layout, levels: &[Vec<u8>]) -> Result<Vec<PathBuf>> {
    let vtf_path = options
        .output_directory
        .join(format!("{}.vtf", options.output));
    std::fs::create_dir_all(&options.output_directory).map_err(|error| {
        ImportError::new(format!(
            "cannot create {}. {error}",
            options.output_directory.display()
        ))
    })?;
    std::fs::write(
        &vtf_path,
        vtf::write(layout, options.format, options.sampling, levels),
    )
    .map_err(|error| ImportError::new(format!("cannot write {}. {error}", vtf_path.display())))?;
    let mut written = vec![vtf_path];
    if options.write_vmt {
        let vmt_path = options
            .output_directory
            .join(format!("{}.vmt", options.output));
        std::fs::write(&vmt_path, vtf::vmt(&options.output)).map_err(|error| {
            ImportError::new(format!("cannot write {}. {error}", vmt_path.display()))
        })?;
        written.push(vmt_path);
    }
    Ok(written)
}

fn validate(options: &SprayOptions) -> Result<()> {
    if options.output.is_empty() {
        return Err(ImportError::new("the output name cannot be empty"));
    }
    if options
        .output
        .chars()
        .any(|character| INVALID_NAME_CHARACTERS.contains(&character))
    {
        return Err(ImportError::new(
            r#"the output name cannot contain any of \ / : * ? " < > |"#,
        ));
    }
    if !options.speed.is_finite() || options.speed <= 0.0 {
        return Err(ImportError::new("--speed must be a positive number"));
    }
    if options.jobs < 1 {
        return Err(ImportError::new("--jobs must be at least 1"));
    }
    if let Some(size) = options.width.or(options.height)
        && size < 4
    {
        return Err(ImportError::new(
            "the texture size must be at least 4 pixels",
        ));
    }
    for spec in &options.mip_specs {
        if spec.paths.is_empty() {
            return Err(ImportError::new(format!(
                "--mip {} needs at least one image",
                spec.level
            )));
        }
    }
    Ok(())
}

fn report_layout(options: &SprayOptions, layout: &Layout, bytes: u64) {
    println!(
        "output      {}",
        options
            .output_directory
            .join(format!("{}.vtf", options.output))
            .display()
    );
    println!("format      {}", options.format.name());
    println!("size        {}x{}", layout.width, layout.height);
    println!("frames      {}", layout.frames);
    println!("mip levels  {}", layout.mip_count + 1);
    println!(
        "estimated   {} (limit {})",
        describe(bytes),
        describe((budget::LIMIT_KIB * 1024.0) as u64)
    );
    println!("threads     {}", options.jobs);
    if options.write_vmt {
        println!(
            "material    {}",
            options
                .output_directory
                .join(format!("{}.vmt", options.output))
                .display()
        );
    }
}

fn report_result(options: &SprayOptions, layout: &Layout, bytes: u64, written: &[PathBuf]) {
    println!(
        "{}  {}  {}x{}  {} frames  {}",
        written[0].display(),
        options.format.name(),
        layout.width,
        layout.height,
        layout.frames,
        describe(bytes),
    );
    for path in &written[1..] {
        println!("{}", path.display());
    }
}

fn describe(bytes: u64) -> String {
    format!("{:.1} KiB", bytes as f64 / 1024.0)
}

fn warn(message: &str) {
    eprintln!("warning: {message}");
}

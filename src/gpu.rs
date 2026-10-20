#[cfg(not(feature = "gpu"))]
use crate::error::{ImportError, Result};
#[cfg(not(feature = "gpu"))]
use crate::format::{Quality, VtfFormat};
#[cfg(not(feature = "gpu"))]
use crate::frame::Frame;

#[cfg(feature = "gpu")]
pub use backend::Encoder;

#[cfg(feature = "gpu")]
mod backend {
    use crate::encode;
    use crate::error::{ImportError, Result};
    use crate::format::{Quality, VtfFormat};
    use crate::frame::Frame;
    use crate::quantise;
    use wgpu::util::DeviceExt;

    const SHADER: &str = include_str!("gpu/shader.wgsl");

    const KIND_DXT1: u32 = 0;
    const KIND_DXT5: u32 = 1;
    const KIND_PACKED: u32 = 2;

    const PARAM_COUNT: usize = 16;

    pub struct Encoder {
        device: wgpu::Device,
        queue: wgpu::Queue,
        bind_group_layout: wgpu::BindGroupLayout,
        blocks: wgpu::ComputePipeline,
        pixels: wgpu::ComputePipeline,
        dither_matrix: wgpu::Buffer,
        shades: wgpu::Buffer,
    }

    impl Encoder {
        pub fn new() -> Result<Self> {
            let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
                backends: wgpu::Backends::from_env().unwrap_or(wgpu::Backends::PRIMARY),
                ..wgpu::InstanceDescriptor::new_without_display_handle_from_env()
            });
            let adapter =
                pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::HighPerformance,
                    compatible_surface: None,
                    force_fallback_adapter: false,
                    ..Default::default()
                }))
                .map_err(|error| {
                    ImportError::new(format!("no GPU adapter was found: {error:?}"))
                })?;
            if adapter.get_info().device_type == wgpu::DeviceType::Cpu {
                return Err(ImportError::new("only a software GPU adapter was found"));
            }
            let (device, queue) =
                pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                    label: Some("leake.encode"),
                    ..Default::default()
                }))
                .map_err(|error| ImportError::new(format!("cannot open the GPU: {error:?}")))?;

            let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
            let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("leake.encode"),
                source: wgpu::ShaderSource::Wgsl(SHADER.into()),
            });
            let bind_group_layout = bind_group_layout(&device);
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("leake.encode"),
                bind_group_layouts: &[Some(&bind_group_layout)],
                immediate_size: 0,
            });
            let blocks = create_pipeline(
                &device,
                &module,
                &pipeline_layout,
                "encode_blocks",
                "leake.encode.blocks",
            );
            let pixels = create_pipeline(
                &device,
                &module,
                &pipeline_layout,
                "encode_pixels",
                "leake.encode.pixels",
            );
            if let Some(error) = pollster::block_on(scope.pop()) {
                return Err(ImportError::new(format!(
                    "the GPU encoder was rejected: {}",
                    brief(error)
                )));
            }

            let mut dither_bytes = Vec::new();
            for row in quantise::DITHER_MATRIX {
                for value in row {
                    dither_bytes.extend_from_slice(&u32::from(value).to_le_bytes());
                }
            }
            let dither_matrix = table_buffer(&device, "leake.encode.dither", &dither_bytes);

            let mut shade_bytes = Vec::new();
            for bits in [1, 4, 5, 6] {
                let (lower, upper) = quantise::shade_tables(bits);
                for value in lower {
                    shade_bytes.extend_from_slice(&value.to_le_bytes());
                }
                for value in upper {
                    shade_bytes.extend_from_slice(&value.to_le_bytes());
                }
            }
            let shades = table_buffer(&device, "leake.encode.shades", &shade_bytes);

            Ok(Self {
                device,
                queue,
                bind_group_layout,
                blocks,
                pixels,
                dither_matrix,
                shades,
            })
        }

        pub fn encode(
            &self,
            raster: &Frame,
            format: VtfFormat,
            quality: Quality,
            dither: bool,
        ) -> Result<Vec<u8>> {
            let blocks_x = raster.width.div_ceil(4);
            let blocks_y = raster.height.div_ceil(4);
            let (kind, words) = match format {
                VtfFormat::Dxt1 => (KIND_DXT1, u64::from(blocks_x) * u64::from(blocks_y) * 2),
                VtfFormat::Dxt5 => (KIND_DXT5, u64::from(blocks_x) * u64::from(blocks_y) * 4),
                _ => (
                    KIND_PACKED,
                    u64::from(raster.width) * u64::from(raster.height),
                ),
            };
            let output_bytes = words * 4;
            let workgroups = if kind == KIND_PACKED {
                (raster.width.div_ceil(8), raster.height.div_ceil(8))
            } else {
                (blocks_x.div_ceil(8), blocks_y.div_ceil(8))
            };
            self.check_limits(raster, output_bytes, workgroups)?;

            let params = make_params(raster, blocks_x, blocks_y, kind, format, quality, dither);
            let input = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("leake.encode.input"),
                    contents: &raster.rgba,
                    usage: wgpu::BufferUsages::STORAGE,
                });
            let output = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("leake.encode.output"),
                size: output_bytes,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });
            let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("leake.encode.readback"),
                size: output_bytes,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let mut param_bytes = Vec::with_capacity(PARAM_COUNT * 4);
            for value in params {
                param_bytes.extend_from_slice(&value.to_le_bytes());
            }
            let uniforms = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("leake.encode.params"),
                    contents: &param_bytes,
                    usage: wgpu::BufferUsages::UNIFORM,
                });
            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("leake.encode"),
                layout: &self.bind_group_layout,
                entries: &[
                    binding(0, &input),
                    binding(1, &output),
                    binding(2, &uniforms),
                    binding(3, &self.dither_matrix),
                    binding(4, &self.shades),
                ],
            });

            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("leake.encode"),
                });
            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("leake.encode"),
                    timestamp_writes: None,
                });
                pass.set_bind_group(0, &bind_group, &[]);
                if kind == KIND_PACKED {
                    pass.set_pipeline(&self.pixels);
                } else {
                    pass.set_pipeline(&self.blocks);
                }
                pass.dispatch_workgroups(workgroups.0, workgroups.1, 1);
            }
            encoder.copy_buffer_to_buffer(&output, 0, &readback, 0, output_bytes);
            self.queue.submit([encoder.finish()]);

            let mapped = self.read_back(&readback, output_bytes)?;
            Ok(decode_result(raster, format, kind, mapped))
        }

        fn check_limits(
            &self,
            raster: &Frame,
            output_bytes: u64,
            workgroups: (u32, u32),
        ) -> Result<()> {
            let input_bytes = raster.rgba.len() as u64;
            let limits = self.device.limits();
            let too_large = output_bytes > limits.max_buffer_size
                || output_bytes > limits.max_storage_buffer_binding_size
                || input_bytes > limits.max_buffer_size
                || input_bytes > limits.max_storage_buffer_binding_size
                || workgroups.0 > limits.max_compute_workgroups_per_dimension
                || workgroups.1 > limits.max_compute_workgroups_per_dimension;
            if too_large {
                return Err(ImportError::new("the texture is too large for this GPU"));
            }
            Ok(())
        }

        fn read_back(&self, readback: &wgpu::Buffer, output_bytes: u64) -> Result<Vec<u8>> {
            let (sender, receiver) = std::sync::mpsc::channel();
            readback
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |result| {
                    let _ = sender.send(result);
                });
            self.device
                .poll(wgpu::PollType::wait_indefinitely())
                .map_err(|error| ImportError::new(format!("the GPU encoder stalled: {error:?}")))?;
            receiver
                .recv()
                .map_err(|_| ImportError::new("the GPU encoder returned no result"))?
                .map_err(|error| {
                    ImportError::new(format!("cannot read the GPU result: {error:?}"))
                })?;
            let mapped = {
                let view = readback.slice(..).get_mapped_range().map_err(|error| {
                    ImportError::new(format!("cannot read the GPU result: {error:?}"))
                })?;
                view.to_vec()
            };
            readback.unmap();
            let length = usize::try_from(output_bytes).expect("output size fits in memory");
            Ok(mapped[..length].to_vec())
        }
    }

    fn bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("leake.encode"),
            entries: &[
                buffer_entry(0, wgpu::BufferBindingType::Storage { read_only: true }),
                buffer_entry(1, wgpu::BufferBindingType::Storage { read_only: false }),
                buffer_entry(2, wgpu::BufferBindingType::Uniform),
                buffer_entry(3, wgpu::BufferBindingType::Storage { read_only: true }),
                buffer_entry(4, wgpu::BufferBindingType::Storage { read_only: true }),
            ],
        })
    }

    fn buffer_entry(binding: u32, ty: wgpu::BufferBindingType) -> wgpu::BindGroupLayoutEntry {
        wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }
    }

    fn binding(binding: u32, buffer: &wgpu::Buffer) -> wgpu::BindGroupEntry<'_> {
        wgpu::BindGroupEntry {
            binding,
            resource: buffer.as_entire_binding(),
        }
    }

    fn create_pipeline(
        device: &wgpu::Device,
        module: &wgpu::ShaderModule,
        layout: &wgpu::PipelineLayout,
        entry_point: &str,
        label: &str,
    ) -> wgpu::ComputePipeline {
        device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some(label),
            layout: Some(layout),
            module,
            entry_point: Some(entry_point),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        })
    }

    fn table_buffer(device: &wgpu::Device, label: &str, contents: &[u8]) -> wgpu::Buffer {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(label),
            contents,
            usage: wgpu::BufferUsages::STORAGE,
        })
    }

    fn make_params(
        raster: &Frame,
        blocks_x: u32,
        blocks_y: u32,
        kind: u32,
        format: VtfFormat,
        quality: Quality,
        dither: bool,
    ) -> [u32; PARAM_COUNT] {
        let mut params = [0u32; PARAM_COUNT];
        params[0] = raster.width;
        params[1] = raster.height;
        params[2] = blocks_x;
        params[3] = blocks_y;
        params[4] = kind;
        params[5] = u32::from(dither);
        params[6] = refine_passes(quality);
        if kind == KIND_PACKED {
            let spec = quantise::spec(format);
            params[7] = spec.red;
            params[8] = spec.green;
            params[9] = spec.blue;
            params[10] = spec.alpha;
            params[11] = u32::from(spec.opaque);
            params[12] = u32::from(spec.true_colour());
            params[13] = u32::from(spec.dithers);
        }
        params
    }

    fn decode_result(raster: &Frame, format: VtfFormat, kind: u32, mapped: Vec<u8>) -> Vec<u8> {
        if kind != KIND_PACKED {
            return mapped;
        }
        let pixel_count = usize::try_from(u64::from(raster.width) * u64::from(raster.height))
            .expect("pixel count fits in memory");
        let bytes_per_pixel = encode::bytes_per_pixel(format);
        let mut result = vec![0u8; pixel_count * bytes_per_pixel];
        for (index, word) in mapped
            .as_chunks::<4>()
            .0
            .iter()
            .take(pixel_count)
            .enumerate()
        {
            let pixel = u32::from_le_bytes(*word);
            encode::write_pixel(pixel, format, &mut result, index * bytes_per_pixel);
        }
        result
    }

    fn refine_passes(quality: Quality) -> u32 {
        match quality {
            Quality::Fast => 0,
            Quality::Normal => 1,
            Quality::Slow => 2,
            Quality::VerySlow => 4,
        }
    }

    fn brief(error: impl std::fmt::Debug) -> String {
        let text = format!("{error:?}");
        let description = text.rsplit("description: ").next().unwrap_or(&text);
        let mut short: String = description.chars().take(300).collect();
        if short.len() < description.len() {
            short.push_str("...");
        }
        short
    }
}

#[cfg(not(feature = "gpu"))]
pub struct Encoder;

#[cfg(not(feature = "gpu"))]
impl Encoder {
    pub fn new() -> Result<Self> {
        Err(ImportError::new(
            "this build was compiled without the gpu feature",
        ))
    }

    pub fn encode(
        &self,
        _raster: &Frame,
        _format: VtfFormat,
        _quality: Quality,
        _dither: bool,
    ) -> Result<Vec<u8>> {
        Err(ImportError::new(
            "this build was compiled without the gpu feature",
        ))
    }
}

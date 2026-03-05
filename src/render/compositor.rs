use super::tile_cache::{Tile, TILE_SIZE};

/// Per-tile GPU data.
struct TileGpuData {
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
}

/// Real wgpu-backed compositor that uploads tile images and renders them as
/// textured quads.
pub struct WgpuCompositor {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    uniform_buffer: wgpu::Buffer,
    tiles: Vec<Option<TileGpuData>>,
    cols: u32,
    rows: u32,
    viewport_width: u32,
    viewport_height: u32,
}

/// Uniform data sent per tile draw call: NDC offset (x,y) and scale (w,h).
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct TileUniforms {
    offset_scale: [f32; 4],
}

impl WgpuCompositor {
    /// Create a new compositor with a real wgpu render pipeline.
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        viewport_width: u32,
        viewport_height: u32,
    ) -> Self {
        let shader_src = include_str!("shaders/tile_composite.wgsl");
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("tile_composite"),
            source: wgpu::ShaderSource::Wgsl(shader_src.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("tile_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("tile_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("tile_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("tile_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("tile_uniforms"),
            size: std::mem::size_of::<TileUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let cols = viewport_width.div_ceil(TILE_SIZE);
        let rows = viewport_height.div_ceil(TILE_SIZE);
        let count = (cols * rows) as usize;
        let mut tiles = Vec::with_capacity(count);
        tiles.resize_with(count, || None);

        Self {
            pipeline,
            bind_group_layout,
            sampler,
            uniform_buffer,
            tiles,
            cols,
            rows,
            viewport_width,
            viewport_height,
        }
    }

    /// Upload a CPU tile's pixel data to the GPU.
    pub fn upload_tile(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        col: u32,
        row: u32,
        tile: &Tile,
    ) {
        let idx = (row * self.cols + col) as usize;
        if idx >= self.tiles.len() {
            return;
        }

        // Create texture if needed
        if self.tiles[idx].is_none() {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("tile_tex"),
                size: wgpu::Extent3d {
                    width: TILE_SIZE,
                    height: TILE_SIZE,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });

            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("tile_bg"),
                layout: &self.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.uniform_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            });

            self.tiles[idx] = Some(TileGpuData {
                texture,
                bind_group,
            });
        }

        // Write pixel data
        if let Some(gpu_tile) = &self.tiles[idx] {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &gpu_tile.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                tile.image.data(),
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(TILE_SIZE * 4),
                    rows_per_image: Some(TILE_SIZE),
                },
                wgpu::Extent3d {
                    width: TILE_SIZE,
                    height: TILE_SIZE,
                    depth_or_array_layers: 1,
                },
            );
        }
    }

    /// Render a frame by compositing all uploaded tiles.
    pub fn render_frame(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface: &wgpu::Surface<'_>,
        _config: &wgpu::SurfaceConfiguration,
    ) -> Result<(), wgpu::SurfaceError> {
        let output = surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("tile_encoder"),
        });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("tile_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });

            pass.set_pipeline(&self.pipeline);

            for row in 0..self.rows {
                for col in 0..self.cols {
                    let idx = (row * self.cols + col) as usize;
                    if let Some(gpu_tile) = &self.tiles[idx] {
                        // Compute NDC position for this tile
                        let ndc_x = (col as f32 * TILE_SIZE as f32) / self.viewport_width as f32
                            * 2.0
                            - 1.0;
                        let ndc_y = 1.0
                            - (row as f32 * TILE_SIZE as f32) / self.viewport_height as f32 * 2.0;
                        let ndc_w = TILE_SIZE as f32 / self.viewport_width as f32 * 2.0;
                        let ndc_h = -(TILE_SIZE as f32 / self.viewport_height as f32 * 2.0);

                        let uniforms = TileUniforms {
                            offset_scale: [ndc_x, ndc_y, ndc_w, ndc_h],
                        };
                        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));

                        pass.set_bind_group(0, &gpu_tile.bind_group, &[]);
                        pass.draw(0..6, 0..1);
                    }
                }
            }
        }

        queue.submit(std::iter::once(encoder.finish()));
        output.present();
        Ok(())
    }

    /// Handle viewport resize: recompute grid, clear GPU tiles.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.viewport_width = width;
        self.viewport_height = height;
        self.cols = width.div_ceil(TILE_SIZE);
        self.rows = height.div_ceil(TILE_SIZE);
        let count = (self.cols * self.rows) as usize;
        self.tiles.clear();
        self.tiles.resize_with(count, || None);
    }

    /// Get the viewport dimensions.
    pub fn viewport_size(&self) -> (u32, u32) {
        (self.viewport_width, self.viewport_height)
    }
}

//! Shared GPU law and private persistent basins for living UI water surfaces.

use bytemuck::{Pod, Zeroable};
use egui_wgpu::wgpu;
use std::{
    collections::HashMap,
    f32::consts::TAU,
    mem::size_of,
    sync::{Arc, Weak},
};

#[cfg(test)]
mod audit;
mod dump;
mod guard;

const LEVELS: usize = 3;
const SIM_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rg32Float;
const SIM_BYTES: u32 = 8;
const SIM_WORKGROUP: u32 = 8;
const FIELD_SCALE: u32 = 2;
const SIM_STEPS: usize = 4;
const SIM_DT: f32 = 1.0 / 240.0;

pub(super) const LIFT_SLOTS: usize = 4;

pub(super) const SPLASH_SLOTS: usize = 32;
pub(super) const QUIVER_SLOTS: usize = 4;
pub const BULGE_CEIL: f32 = 12.0;
pub(super) const TOUCH_SLOTS: usize = 12;

#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C)]
pub struct Chemistry {
    pub reach: f32,
    pub meniscus_px: f32,
    pub refract_px: f32,
    pub ior_spread: f32,
    pub quiver_bulge: f32,
    pub quiver_pulse: f32,
    pub tremor_k: f32,
    pub tremor_omega: f32,
    pub tremor_amp: f32,
    pub tremor_fade: f32,
    pub tremor_reach: f32,
    pub bulge_px: f32,
    pub lift_bright: f32,
    pub wave_v: f32,
    pub wave_sigma: f32,
    pub wave_damp: f32,
    pub wave_spread: f32,
    pub source_gain: f32,
    pub height_retention: f32,
    pub tilt_gain: f32,
    pub t_panel: f32,
    pub r_panel: f32,
    pub r_wall: f32,
    pub shore_feather: f32,
}

impl Default for Chemistry {
    fn default() -> Self {
        Self {
            reach: 34.0,
            meniscus_px: 1.4,
            refract_px: 1.0,
            ior_spread: 0.34,
            quiver_bulge: 3.0,
            quiver_pulse: 0.2,
            tremor_k: 0.2417,
            tremor_omega: 0.9 * TAU,
            tremor_amp: 0.18,
            tremor_fade: 55.0,
            tremor_reach: 150.0,
            bulge_px: 10.0,
            lift_bright: 0.08,
            wave_v: 320.0,
            wave_sigma: 14.0,
            wave_damp: 2.4,
            wave_spread: 480.0,
            source_gain: 44.0,
            height_retention: 0.99965,
            tilt_gain: 120.0,
            t_panel: 0.12,
            r_panel: 0.35,
            r_wall: 0.6,
            shore_feather: 12.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Tension {
    pub(super) rect: egui::Rect,
    pub(super) pointer: egui::Pos2,
    pub(super) grip: f32,
    pub(super) omega: f32,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Lift {
    pub(super) rect: egui::Rect,
    pub(super) grip: f32,
    pub(super) depth: LiftDepth,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum LiftDepth {
    Surface,
    Shallow,
}

impl Lift {
    pub fn surface(rect: egui::Rect, grip: f32) -> Self {
        Self {
            rect,
            grip,
            depth: LiftDepth::Surface,
        }
    }

    pub fn shallow(rect: egui::Rect, grip: f32) -> Self {
        Self {
            rect,
            grip,
            depth: LiftDepth::Shallow,
        }
    }

    fn packed_grip(self) -> f32 {
        let grip = self.grip.clamp(0.0, 1.0);
        match self.depth {
            LiftDepth::Surface => grip,
            LiftDepth::Shallow => -grip,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Splash {
    pub(super) rect: egui::Rect,
    pub(super) age: f32,
    pub(super) amp: f32,
    pub(super) shape: SplashShape,
    /// Signed screen-y a dragged surface (a spun tape) travels, 0 for a plain
    /// splash. Non-zero turns the source into a directional velocity dipole.
    pub(super) drag: f32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) enum SplashShape {
    #[default]
    Ring,
    Basin,
    /// A broadband velocity-noise impulse over the whole rect — the sheet
    /// "thwacked" from beneath. The solver's KO term shreds the high-k content
    /// into a quick shimmer; the sparse low-k residue rides out.
    Jitter,
}

impl SplashShape {
    fn code(self) -> f32 {
        self as u8 as f32
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Raft {
    pub(super) rect: egui::Rect,
    pub(super) corners: [f32; 4],
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Touch {
    pub(super) center: egui::Pos2,
    pub(super) age: f32,
    pub(super) amp: f32,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Domain {
    pub(super) rect: egui::Rect,
    pub(super) enclosed: f32,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Floor {
    pub(super) rect: egui::Rect,
    pub(super) depth: f32,
}

impl Floor {
    pub(super) const NONE: Self = Self {
        rect: egui::Rect {
            min: egui::Pos2 { x: -4e6, y: -4e6 },
            max: egui::Pos2 { x: -4e6, y: -4e6 },
        },
        depth: 0.0,
    };
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Veil {
    pub(super) cuts: [Cut; 2],
    pub(super) strength: f32,
    pub(super) dim: f32,
    pub(super) blur: f32,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Cut {
    pub(super) rect: egui::Rect,
    pub(super) radius: f32,
    pub(super) barrier: f32,
}

impl Cut {
    /// A cutout that never matches any pixel: uniform blur.
    pub const NONE: Self = Self {
        rect: egui::Rect {
            min: egui::Pos2 { x: -4e6, y: -4e6 },
            max: egui::Pos2 { x: -4e6, y: -4e6 },
        },
        radius: 0.0,
        barrier: 0.0,
    };
}

/// Shared compute and optical machinery. Persistent state lives in one private
/// basin per application [`super::Surface`].
pub struct Engine {
    sample_layout: wgpu::BindGroupLayout,
    composite_layout: wgpu::BindGroupLayout,
    sim_layout: wgpu::BindGroupLayout,
    down: wgpu::RenderPipeline,
    up: wgpu::RenderPipeline,
    pipelines: Pipelines,
    sampler: wgpu::Sampler,
    format: wgpu::TextureFormat,
    viewport: Option<Viewport>,
}

struct Pipelines {
    composite: wgpu::RenderPipeline,
    sim: wgpu::ComputePipeline,
}

/// The size-dependent resources, rebuilt on resize.
struct Viewport {
    size: wgpu::Extent3d,
    scene: Target,
    blur_chain: Vec<Target>,
    basins: HashMap<super::surface::SurfaceId, Basin>,
}

/// One persistent `(height, velocity)` field and its numerical guard.
struct Basin {
    size: wgpu::Extent3d,
    life: Weak<()>,
    uniforms: wgpu::Buffer,
    textures: [wgpu::Texture; 2],
    composite_bindings: [wgpu::BindGroup; 2],
    sim_bindings: [wgpu::BindGroup; 2],
    phase: usize,
    generation: u64,
    sentinel: guard::Sentinel,
}

struct Target {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    /// Bind group for passes that sample this target.
    bind: wgpu::BindGroup,
}

/// Resources needed to mint a basin inside one viewport.
struct Foundry<'a> {
    device: &'a wgpu::Device,
    queue: &'a wgpu::Queue,
    viewport_size: wgpu::Extent3d,
    scene: &'a Target,
    blur: &'a Target,
    composite_layout: &'a wgpu::BindGroupLayout,
    sim_layout: &'a wgpu::BindGroupLayout,
    sampler: &'a wgpu::Sampler,
}

/// CPU/WGSL treaty. Every aggregate begins on a 16-byte boundary; the size
/// assertion and bytemuck cast make layout drift a compile-time failure rather
/// than a chromatic scar discovered by eye.
#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct Uniforms {
    cut_rects: [[f32; 4]; 2],
    cut_vitals: [[f32; 4]; 2],
    domain: [f32; 4],
    optics: [f32; 4],
    motion: [f32; 4],
    lift_rects: [[f32; 4]; LIFT_SLOTS],
    lift_grips: [f32; 4],
    quivers: [[f32; 8]; QUIVER_SLOTS],
    splashes: [[f32; 8]; SPLASH_SLOTS],
    pond: [f32; 4],
    touches: [[f32; 4]; TOUCH_SLOTS],
    chemistry: Chemistry,
    raft_rect: [f32; 4],
    raft_corners: [f32; 4],
    floor_rect: [f32; 4],
    floor_vitals: [f32; 4],
}

const UNIFORM_BYTES: u64 = size_of::<Uniforms>() as u64;
const _: () = assert!(size_of::<Uniforms>() == 1712);

impl Engine {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("poolrooms-water"),
            source: wgpu::ShaderSource::Wgsl(WGSL.into()),
        });
        let sim_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("poolrooms-water-sim"),
            source: wgpu::ShaderSource::Wgsl(SIM_WGSL.into()),
        });
        let sample_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("poolrooms-water-sample"),
            entries: &[texture_entry(0), sampler_entry(1)],
        });
        let composite_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("poolrooms-water-composite"),
            entries: &[
                texture_entry(0),
                texture_entry(1),
                sampler_entry(2),
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(UNIFORM_BYTES),
                    },
                    count: None,
                },
                unfilterable_texture_entry(4, wgpu::ShaderStages::FRAGMENT),
            ],
        });
        let sim_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("poolrooms-water-sim"),
            entries: &[
                unfilterable_texture_entry(0, wgpu::ShaderStages::COMPUTE),
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: SIM_FORMAT,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(UNIFORM_BYTES),
                    },
                    count: None,
                },
            ],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("poolrooms-water-linear"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let pipeline =
            |label: &str, layout: &wgpu::BindGroupLayout, entry, constants: &[(&str, f64)]| {
                let pipeline_layout =
                    device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some(label),
                        bind_group_layouts: &[Some(layout)],
                        immediate_size: 0,
                    });
                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some(label),
                    layout: Some(&pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &module,
                        entry_point: Some("fullscreen"),
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                        buffers: &[],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &module,
                        entry_point: Some(entry),
                        compilation_options: wgpu::PipelineCompilationOptions {
                            constants,
                            ..Default::default()
                        },
                        targets: &[Some(wgpu::ColorTargetState {
                            format,
                            blend: None,
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                    }),
                    primitive: wgpu::PrimitiveState::default(),
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState::default(),
                    multiview_mask: None,
                    cache: None,
                })
            };
        let sim_layout_handle = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("poolrooms-water-sim"),
            bind_group_layouts: &[Some(&sim_layout)],
            immediate_size: 0,
        });
        let field = [("FIELD_SCALE", f64::from(FIELD_SCALE))];
        let sim_consts = [
            ("SIM_SCALE", f64::from(FIELD_SCALE)),
            ("DT", f64::from(SIM_DT)),
            ("IMPULSE_GAIN", 4.0 / SIM_STEPS as f64),
        ];
        let pipelines = Pipelines {
            composite: pipeline(
                "poolrooms-water-composite",
                &composite_layout,
                "composite",
                &field,
            ),
            sim: device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("poolrooms-water-sim"),
                layout: Some(&sim_layout_handle),
                module: &sim_module,
                entry_point: Some("step"),
                compilation_options: wgpu::PipelineCompilationOptions {
                    constants: &sim_consts,
                    ..Default::default()
                },
                cache: None,
            }),
        };
        Self {
            down: pipeline("poolrooms-water-down", &sample_layout, "kawase_down", &[]),
            up: pipeline("poolrooms-water-up", &sample_layout, "kawase_up", &[]),
            pipelines,
            sample_layout,
            composite_layout,
            sim_layout,
            sampler,
            format,
            viewport: None,
        }
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if width == 0 || height == 0 {
            self.viewport = None;
            return;
        }
        let target = |label: &str, w: u32, h: u32| {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: w.max(1),
                    height: h.max(1),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[self.format],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(label),
                layout: &self.sample_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            });
            Target {
                texture,
                view,
                bind,
            }
        };
        let scene = target("poolrooms-water-scene", width, height);
        let blur_chain = (1..=LEVELS as u32)
            .map(|level| target("poolrooms-water-chain", width >> level, height >> level))
            .collect::<Vec<_>>();
        self.viewport = Some(Viewport {
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            scene,
            blur_chain,
            basins: HashMap::new(),
        });
    }

    /// Render target for the egui pass when a veil is up.
    pub fn scene_view(&self) -> Option<&wgpu::TextureView> {
        self.viewport.as_ref().map(|viewport| &viewport.scene.view)
    }

    /// Composites the offscreen scene to `target` with the sealed water frame.
    pub fn compose(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        frame: &super::Frame,
    ) {
        self.ensure_basin(device, queue, frame.surface, frame.generation, &frame.life);
        let Some(viewport) = &mut self.viewport else {
            return;
        };
        let Some(basin) = viewport.basins.get_mut(&frame.surface) else {
            return;
        };
        queue.write_buffer(&basin.uniforms, 0, bytemuck::bytes_of(&uniforms(frame)));
        if frame.sim_live() {
            for _ in 0..SIM_STEPS {
                run_compute(
                    encoder,
                    &self.pipelines.sim,
                    &basin.sim_bindings[basin.phase],
                    basin.size,
                );
                basin.phase ^= 1;
            }
        }
        if frame.veil.is_some_and(|veil| veil.blur > 0.0) {
            let mut blur = |pipeline, source: &Target, sink: &wgpu::TextureView| {
                run_pass(encoder, pipeline, &source.bind, sink);
            };
            blur(&self.down, &viewport.scene, &viewport.blur_chain[0].view);
            for level in 1..LEVELS {
                blur(
                    &self.down,
                    &viewport.blur_chain[level - 1],
                    &viewport.blur_chain[level].view,
                );
            }
            for level in (1..LEVELS).rev() {
                blur(
                    &self.up,
                    &viewport.blur_chain[level],
                    &viewport.blur_chain[level - 1].view,
                );
            }
        }
        run_pass(
            encoder,
            &self.pipelines.composite,
            &basin.composite_bindings[basin.phase],
            target,
        );
        if frame.guard {
            basin
                .sentinel
                .encode(device, encoder, basin.size, &basin.textures[basin.phase]);
        }
    }

    pub fn after_submit(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        frame: &super::Frame,
    ) -> bool {
        let Some(viewport) = &mut self.viewport else {
            return false;
        };
        let Some(basin) = viewport.basins.get_mut(&frame.surface) else {
            return false;
        };
        if frame.guard {
            let poisoned = basin.sentinel.after_submit(device);
            if poisoned {
                basin.clear(queue);
            }
            poisoned
        } else {
            basin.sentinel.disarm();
            false
        }
    }

    pub fn becalm(&mut self, queue: &wgpu::Queue) {
        if let Some(viewport) = &mut self.viewport {
            for basin in viewport.basins.values_mut() {
                basin.sentinel.disarm();
                basin.clear(queue);
            }
        }
    }

    fn ensure_basin(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        id: super::surface::SurfaceId,
        generation: u64,
        life: &Arc<()>,
    ) {
        let Some(viewport) = &mut self.viewport else {
            return;
        };
        let Viewport {
            size,
            scene,
            blur_chain,
            basins,
        } = viewport;
        basins.retain(|_, basin| basin.life.strong_count() != 0);
        let basin = basins.entry(id).or_insert_with(|| {
            Foundry {
                device,
                queue,
                viewport_size: *size,
                scene,
                blur: &blur_chain[0],
                composite_layout: &self.composite_layout,
                sim_layout: &self.sim_layout,
                sampler: &self.sampler,
            }
            .basin(generation, Arc::downgrade(life))
        });
        if basin.generation != generation {
            basin.sentinel.disarm();
            basin.clear(queue);
            basin.phase = 0;
            basin.generation = generation;
        }
    }
}

impl Foundry<'_> {
    fn basin(self, generation: u64, life: Weak<()>) -> Basin {
        let Self {
            device,
            queue,
            viewport_size,
            scene,
            blur,
            composite_layout,
            sim_layout,
            sampler,
        } = self;
        let size = wgpu::Extent3d {
            width: viewport_size.width.div_ceil(FIELD_SCALE).max(1),
            height: viewport_size.height.div_ceil(FIELD_SCALE).max(1),
            depth_or_array_layers: 1,
        };
        let textures = std::array::from_fn(|_| {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("poolrooms-water-field"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: SIM_FORMAT,
                usage: wgpu::TextureUsages::COPY_DST
                    | wgpu::TextureUsages::COPY_SRC
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::STORAGE_BINDING,
                view_formats: &[SIM_FORMAT],
            });
            let zeros = vec![0_u8; (size.width * size.height * SIM_BYTES) as usize];
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &zeros,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(size.width * SIM_BYTES),
                    rows_per_image: Some(size.height),
                },
                size,
            );
            texture
        });
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("poolrooms-water-basin-uniforms"),
            size: UNIFORM_BYTES,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let views = textures
            .each_ref()
            .map(|texture| texture.create_view(&wgpu::TextureViewDescriptor::default()));
        let composite_bindings = views.each_ref().map(|view| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("poolrooms-water-composite"),
                layout: composite_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&scene.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&blur.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: uniforms.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::TextureView(view),
                    },
                ],
            })
        });
        let sim_bindings = std::array::from_fn(|src| {
            let dst = src ^ 1;
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("poolrooms-water-sim"),
                layout: sim_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&views[src]),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&views[dst]),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: uniforms.as_entire_binding(),
                    },
                ],
            })
        });
        Basin {
            size,
            life,
            uniforms,
            textures,
            composite_bindings,
            sim_bindings,
            phase: 0,
            generation,
            sentinel: guard::Sentinel::default(),
        }
    }
}

fn run_pass(
    encoder: &mut wgpu::CommandEncoder,
    pipeline: &wgpu::RenderPipeline,
    bind: &wgpu::BindGroup,
    sink: &wgpu::TextureView,
) {
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("poolrooms-water"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: sink,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                store: wgpu::StoreOp::Store,
            },
            depth_slice: None,
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, bind, &[]);
    pass.draw(0..3, 0..1);
}

fn run_compute(
    encoder: &mut wgpu::CommandEncoder,
    pipeline: &wgpu::ComputePipeline,
    bind: &wgpu::BindGroup,
    size: wgpu::Extent3d,
) {
    let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
        label: Some("poolrooms-water-sim"),
        timestamp_writes: None,
    });
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, bind, &[]);
    pass.dispatch_workgroups(
        size.width.div_ceil(SIM_WORKGROUP),
        size.height.div_ceil(SIM_WORKGROUP),
        1,
    );
}

fn texture_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

fn unfilterable_texture_entry(
    binding: u32,
    visibility: wgpu::ShaderStages,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: false },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

fn sampler_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        count: None,
    }
}

fn uniforms(frame: &super::Frame) -> Uniforms {
    const NO_VEIL: Veil = Veil {
        cuts: [Cut::NONE, Cut::NONE],
        strength: 0.0,
        dim: 1.0,
        blur: 0.0,
    };
    let veil = frame.veil.unwrap_or(NO_VEIL);
    let mut packed = Uniforms::zeroed();
    for (slot, cut) in veil.cuts.iter().enumerate() {
        packed.cut_rects[slot] = [
            cut.rect.min.x,
            cut.rect.min.y,
            cut.rect.max.x,
            cut.rect.max.y,
        ];
        packed.cut_vitals[slot] = [cut.radius, cut.barrier, 0.0, 0.0];
    }
    packed.domain = [
        frame.domain.rect.min.x,
        frame.domain.rect.min.y,
        frame.domain.rect.max.x,
        frame.domain.rect.max.y,
    ];
    packed.optics = [
        veil.strength.clamp(0.0, 1.0),
        veil.dim,
        veil.blur.clamp(0.0, 1.0),
        frame.tide,
    ];
    packed.motion = [frame.scroll_tilt, frame.domain.enclosed, 0.0, 0.0];
    for (slot, lift) in frame.lifts.iter().take(LIFT_SLOTS).enumerate() {
        packed.lift_rects[slot] = [
            lift.rect.min.x,
            lift.rect.min.y,
            lift.rect.max.x,
            lift.rect.max.y,
        ];
        packed.lift_grips[slot] = lift.packed_grip();
    }
    for (slot, quiver) in frame.tensions.iter().take(QUIVER_SLOTS).enumerate() {
        packed.quivers[slot] = [
            quiver.rect.min.x,
            quiver.rect.min.y,
            quiver.rect.max.x,
            quiver.rect.max.y,
            quiver.pointer.x,
            quiver.pointer.y,
            quiver.grip.clamp(0.0, 1.0),
            quiver.omega.max(0.0),
        ];
    }
    for (slot, splash) in frame.splashes.iter().take(SPLASH_SLOTS).enumerate() {
        packed.splashes[slot] = [
            splash.rect.min.x,
            splash.rect.min.y,
            splash.rect.max.x,
            splash.rect.max.y,
            splash.age,
            splash.amp,
            splash.shape.code(),
            splash.drag,
        ];
    }
    packed.pond = [
        frame.viewer.min.x,
        frame.viewer.min.y,
        frame.viewer.max.x,
        frame.viewer.max.y,
    ];
    for (slot, touch) in frame.touches.iter().take(TOUCH_SLOTS).enumerate() {
        packed.touches[slot] = [touch.center.x, touch.center.y, touch.age, touch.amp];
    }
    packed.chemistry = frame.chemistry;
    packed.chemistry.bulge_px = packed.chemistry.bulge_px.min(BULGE_CEIL);
    if let Some(raft) = frame.raft {
        packed.raft_rect = [
            raft.rect.min.x,
            raft.rect.min.y,
            raft.rect.max.x,
            raft.rect.max.y,
        ];
        packed.raft_corners = raft.corners;
    }
    packed.floor_rect = [
        frame.floor.rect.min.x,
        frame.floor.rect.min.y,
        frame.floor.rect.max.x,
        frame.floor.rect.max.y,
    ];
    packed.floor_vitals = [frame.floor.depth.clamp(0.0, 1.0), 0.0, 0.0, 0.0];
    packed
}

const WGSL: &str = concat!(
    include_str!("engine/forcing.wgsl"),
    include_str!("engine/composite.wgsl")
);
const SIM_WGSL: &str = concat!(
    include_str!("engine/forcing.wgsl"),
    include_str!("engine/sim.wgsl")
);

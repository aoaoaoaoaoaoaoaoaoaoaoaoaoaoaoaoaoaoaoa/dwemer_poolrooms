use super::super::Frame;
use super::*;
use anyhow::{Context as _, Result, bail};
use std::{
    fs,
    future::Future,
    process,
    sync::{Arc, Mutex, MutexGuard, PoisonError, mpsc},
    time::Duration,
};

const TARGET_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const W: u32 = 640;
const H: u32 = 360;
static GPU_AUDIT: Mutex<()> = Mutex::new(());

fn audit(future: impl Future<Output = Result<()>>) -> Result<()> {
    let _lease: MutexGuard<'static, ()> = GPU_AUDIT.lock().unwrap_or_else(PoisonError::into_inner);
    pollster::block_on(future)
}

#[test]
fn poisoned_water_cells_are_extinguished() -> Result<()> {
    audit(async {
        let Some(mut bench) = Bench::make().await? else {
            return Ok(());
        };
        bench.poison(37, 29)?;
        bench.step(&quiet(0.0))?;
        let field = bench.field()?;
        field.assert_clean()?;
        field.assert_quiet(37, 29, 0.25)
    })
}

#[test]
fn water_guard_resets_poisoned_field() -> Result<()> {
    audit(async {
        let Some(mut bench) = Bench::make().await? else {
            return Ok(());
        };
        bench.poison(37, 29)?;
        if bench.field()?.assert_clean().is_ok() {
            bail!("basin poison write did not land");
        }
        if !bench.guard()? {
            bail!("basin guard did not report a reset");
        }
        let field = bench.field()?;
        field.assert_clean()?;
        field.assert_quiet(37, 29, 0.25)
    })
}

#[test]
fn water_guard_resets_saturated_field() -> Result<()> {
    audit(async {
        let Some(mut bench) = Bench::make().await? else {
            return Ok(());
        };
        bench.saturate()?;
        bench.field()?.assert_railed(512)?;
        if !bench.guard()? {
            bail!("basin guard did not report a saturated reset");
        }
        let field = bench.field()?;
        field.assert_clean()?;
        field.assert_quiet(37, 29, 0.25)
    })
}

#[test]
fn becalm_zeros_every_persistent_field() -> Result<()> {
    audit(async {
        let Some(mut bench) = Bench::make().await? else {
            return Ok(());
        };
        bench.saturate()?;
        bench.field()?.assert_railed(512)?;
        bench.engine.becalm(&bench.queue);
        let field = bench.field()?;
        field.assert_clean()?;
        field.assert_quiet(37, 29, 0.25)
    })
}

#[test]
fn aggressive_water_script_never_writes_nonfinite_state() -> Result<()> {
    audit(async {
        let Some(mut bench) = Bench::make().await? else {
            return Ok(());
        };
        for frame in 0..180 {
            let script = Script::storm(frame);
            bench.step(&script.frame(frame as f32 / 60.0))?;
            if frame % 15 == 0 {
                bench.field()?.assert_clean()?;
            }
        }
        bench.field()?.assert_clean()
    })
}

#[test]
fn overlapping_image_plate_wakes_do_not_rail_field() -> Result<()> {
    audit(async {
        let Some(mut bench) = Bench::make().await? else {
            return Ok(());
        };
        for frame in 0..180 {
            let script = Script::spaz(frame);
            bench.step(&script.very_wet_frame(frame as f32 / 60.0))?;
        }
        let field = bench.field()?;
        field.assert_clean()?;
        field.assert_not_railed(512)
    })
}

#[test]
fn water_allocates_half_resolution_field() -> Result<()> {
    audit(async {
        let Some(bench) = Bench::make().await? else {
            return Ok(());
        };
        bench.assert_size(W.div_ceil(2), H.div_ceil(2))
    })
}

#[test]
fn surfaces_never_share_a_persistent_basin() -> Result<()> {
    audit(async {
        let Some(mut bench) = Bench::make().await? else {
            return Ok(());
        };
        bench.poison(37, 29)?;
        if bench.field()?.assert_clean().is_ok() {
            bail!("poison did not land in the first basin");
        }
        let stranger = super::super::surface::SurfaceId(2);
        let stranger_life = Arc::new(());
        bench
            .engine
            .ensure_basin(&bench.device, &bench.queue, stranger, 0, &stranger_life);
        let field = bench.field_for(stranger)?;
        field.assert_clean()?;
        field.assert_quiet(37, 29, 0.0)
    })
}

#[test]
fn one_submission_keeps_each_surface_forcing() -> Result<()> {
    audit(async {
        let Some(mut bench) = Bench::make().await? else {
            return Ok(());
        };
        let stranger = super::super::surface::SurfaceId(2);
        let stirred = Script::spaz(0).frame(0.0);
        let quiet = quiet_for(stranger, 0.0);
        let output = bench.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("basin-audit-output"),
            size: wgpu::Extent3d {
                width: W,
                height: H,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: TARGET_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[TARGET_FORMAT],
        });
        let view = output.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = bench
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("basin-audit-multi-surface"),
            });
        bench
            .engine
            .compose(&bench.device, &bench.queue, &mut encoder, &view, &stirred);
        bench
            .engine
            .compose(&bench.device, &bench.queue, &mut encoder, &view, &quiet);
        let ticket = bench.queue.submit([encoder.finish()]);
        let _drained = bench
            .device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(ticket),
                timeout: Some(Duration::from_secs(10)),
            })
            .context("wait multi-surface composition")?;

        bench.field()?.assert_stirred(0.01)?;
        bench.field_for(stranger)?.assert_quiet_everywhere(1.0e-6)
    })
}

#[test]
fn surface_generation_erases_only_its_basin() -> Result<()> {
    audit(async {
        let Some(mut bench) = Bench::make().await? else {
            return Ok(());
        };
        bench.saturate()?;
        bench.field()?.assert_railed(512)?;
        bench
            .engine
            .ensure_basin(&bench.device, &bench.queue, bench.surface, 1, &bench.life);
        let field = bench.field()?;
        field.assert_clean()?;
        field.assert_quiet(37, 29, 0.0)
    })
}

#[test]
fn dead_surfaces_relinquish_their_gpu_basins() -> Result<()> {
    audit(async {
        let Some(mut bench) = Bench::make().await? else {
            return Ok(());
        };
        let doomed = super::super::surface::SurfaceId(2);
        let doomed_life = Arc::new(());
        bench
            .engine
            .ensure_basin(&bench.device, &bench.queue, doomed, 0, &doomed_life);
        drop(doomed_life);

        let survivor = super::super::surface::SurfaceId(3);
        let survivor_life = Arc::new(());
        bench
            .engine
            .ensure_basin(&bench.device, &bench.queue, survivor, 0, &survivor_life);
        let basins = &bench
            .engine
            .viewport
            .as_ref()
            .context("missing water viewport")?
            .basins;
        if basins.contains_key(&doomed) {
            bail!("dead surface retained its GPU basin");
        }
        if !basins.contains_key(&survivor) {
            bail!("live surface lost its GPU basin");
        }
        Ok(())
    })
}

#[test]
fn water_dump_writes_forensic_sections() -> Result<()> {
    audit(async {
        let Some(mut bench) = Bench::make().await? else {
            return Ok(());
        };
        let frame = quiet(0.125);
        bench.step(&frame)?;
        let path = std::env::temp_dir().join(format!(
            "abv-basin-dump-test-{}-{}.abvdump",
            process::id(),
            W
        ));
        let _gone = fs::remove_file(&path);
        bench
            .engine
            .dump(&bench.device, &bench.queue, &path, &frame, [W, H], 1.0)?;
        let blob = fs::read(&path).with_context(|| format!("read {}", path.display()))?;
        fs::remove_file(&path).with_context(|| format!("remove {}", path.display()))?;
        if !blob.starts_with(b"DWEMER_WATER_DUMP\0") {
            bail!("basin dump missing magic header");
        }
        for needle in [b"meta.txt".as_slice(), b"water0.rg32f", b"water1.rg32f"] {
            if !blob.windows(needle.len()).any(|window| window == needle) {
                bail!(
                    "basin dump missing section {}",
                    String::from_utf8_lossy(needle)
                );
            }
        }
        Ok(())
    })
}

struct Bench {
    device: wgpu::Device,
    queue: wgpu::Queue,
    engine: Engine,
    surface: super::super::surface::SurfaceId,
    life: Arc<()>,
}

impl Bench {
    async fn make() -> Result<Option<Self>> {
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
        desc.backends = wgpu::Backends::PRIMARY;
        let instance = wgpu::Instance::new(desc);
        let adapter = match instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
        {
            Ok(adapter) => adapter,
            Err(err) => {
                eprintln!("basin audit skipped: no wgpu adapter: {err}");
                return Ok(None);
            }
        };
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("basin-audit"),
                ..Default::default()
            })
            .await
            .context("request basin audit device")?;
        let mut engine = Engine::new(&device, TARGET_FORMAT);
        engine.resize(&device, W, H);
        let surface = super::super::surface::SurfaceId(1);
        let life = Arc::new(());
        engine.ensure_basin(&device, &queue, surface, 0, &life);
        Ok(Some(Self {
            device,
            queue,
            engine,
            surface,
            life,
        }))
    }

    fn step(&mut self, frame: &Frame) -> Result<()> {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("basin-audit-step"),
            });
        let Engine {
            pipelines,
            viewport,
            ..
        } = &mut self.engine;
        let basin = viewport
            .as_mut()
            .context("missing water viewport")?
            .basins
            .get_mut(&self.surface)
            .context("missing audit basin")?;
        self.queue
            .write_buffer(&basin.uniforms, 0, bytemuck::bytes_of(&uniforms(frame)));
        for _ in 0..SIM_STEPS {
            run_compute(
                &mut encoder,
                &pipelines.sim,
                &basin.sim_bindings[basin.phase],
                basin.size,
            );
            basin.phase ^= 1;
        }
        let ticket = self.queue.submit([encoder.finish()]);
        let _drained = self
            .device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(ticket),
                timeout: Some(Duration::from_secs(10)),
            })
            .context("wait basin audit step")?;
        Ok(())
    }

    fn poison(&mut self, x: u32, y: u32) -> Result<()> {
        let basin = self.basin()?;
        let bits = [f32::INFINITY.to_le_bytes(), f32::INFINITY.to_le_bytes()].concat();
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &basin.textures[basin.phase],
                mip_level: 0,
                origin: wgpu::Origin3d { x, y, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            &bits,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: None,
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        Ok(())
    }

    fn saturate(&mut self) -> Result<()> {
        let basin = self.basin()?;
        let size = basin.size;
        let mut bytes = Vec::with_capacity((size.width * size.height * SIM_BYTES) as usize);
        for y in 0..size.height {
            for x in 0..size.width {
                let sign = if (x + y).is_multiple_of(2) { 1.0 } else { -1.0 };
                bytes.extend_from_slice(&(sign * 48.0_f32).to_le_bytes());
                bytes.extend_from_slice(&(sign * 1440.0_f32).to_le_bytes());
            }
        }
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &basin.textures[basin.phase],
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(size.width * SIM_BYTES),
                rows_per_image: Some(size.height),
            },
            size,
        );
        Ok(())
    }

    fn field(&self) -> Result<Field> {
        self.field_for(self.surface)
    }

    fn field_for(&self, surface: super::super::surface::SurfaceId) -> Result<Field> {
        let basin = self
            .engine
            .viewport
            .as_ref()
            .context("missing water viewport")?
            .basins
            .get(&surface)
            .context("missing audit basin")?;
        Field::read(&self.device, &self.queue, basin)
    }

    fn guard(&mut self) -> Result<bool> {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("basin-audit-guard"),
            });
        let surface = self.surface;
        let basin = self
            .engine
            .viewport
            .as_mut()
            .context("missing water viewport")?
            .basins
            .get_mut(&surface)
            .context("missing audit basin")?;
        basin.sentinel.encode(
            &self.device,
            &mut encoder,
            basin.size,
            &basin.textures[basin.phase],
        );
        let submitted = self.queue.submit([encoder.finish()]);
        let frame = quiet_for(surface, 0.0);
        if self.engine.after_submit(&self.device, &self.queue, &frame) {
            return Ok(true);
        }
        for _ in 0..20 {
            let _drained = self
                .device
                .poll(wgpu::PollType::Wait {
                    submission_index: Some(submitted.clone()),
                    timeout: Some(Duration::from_millis(50)),
                })
                .context("wait basin guard readback")?;
            if self.engine.after_submit(&self.device, &self.queue, &frame) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn assert_size(&self, width: u32, height: u32) -> Result<()> {
        let size = self.basin()?.size;
        if (size.width, size.height) != (width, height) {
            bail!(
                "basin is {}×{}, expected {width}×{height}",
                size.width,
                size.height
            );
        }
        Ok(())
    }

    fn basin(&self) -> Result<&Basin> {
        self.engine
            .viewport
            .as_ref()
            .context("missing water viewport")?
            .basins
            .get(&self.surface)
            .context("missing audit basin")
    }
}

struct Field {
    bytes: Vec<u8>,
    width: u32,
}

impl Field {
    fn read(device: &wgpu::Device, queue: &wgpu::Queue, basin: &Basin) -> Result<Self> {
        let row = basin.size.width * SIM_BYTES;
        let pitch =
            row.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("basin-audit-readback"),
            size: u64::from(pitch) * u64::from(basin.size.height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("basin-audit-readback"),
        });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &basin.textures[basin.phase],
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(pitch),
                    rows_per_image: Some(basin.size.height),
                },
            },
            basin.size,
        );
        let ticket = queue.submit([encoder.finish()]);
        let slice = buffer.slice(..);
        let (tx, rx) = mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _sent = tx.send(result);
        });
        let _drained = device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(ticket),
                timeout: Some(Duration::from_secs(10)),
            })
            .context("wait basin audit readback")?;
        rx.recv_timeout(Duration::from_secs(10))
            .context("receive basin audit map result")?
            .context("map basin audit readback")?;
        let view = slice.get_mapped_range();
        let mut bytes = Vec::with_capacity((row * basin.size.height) as usize);
        for y in 0..basin.size.height {
            let start = (y * pitch) as usize;
            bytes.extend_from_slice(&view[start..start + row as usize]);
        }
        drop(view);
        buffer.unmap();
        Ok(Self {
            bytes,
            width: basin.size.width,
        })
    }

    fn assert_clean(&self) -> Result<()> {
        for (cell, chunk) in self.bytes.chunks_exact(SIM_BYTES as usize).enumerate() {
            for channel in 0..2 {
                let at = channel * 4;
                let value =
                    f32::from_le_bytes([chunk[at], chunk[at + 1], chunk[at + 2], chunk[at + 3]]);
                if !value.is_finite() {
                    bail!(
                        "nonfinite f32 in basin field at ({}, {}), channel {}, bits=0x{:08x}",
                        cell as u32 % self.width,
                        cell as u32 / self.width,
                        channel,
                        value.to_bits(),
                    );
                }
            }
        }
        Ok(())
    }

    fn assert_railed(&self, min_cells: usize) -> Result<()> {
        let cells = self.railed_cells();
        if cells < min_cells {
            bail!("only {cells} railed cells, expected at least {min_cells}");
        }
        Ok(())
    }

    fn assert_not_railed(&self, max_cells: usize) -> Result<()> {
        let cells = self.railed_cells();
        if cells > max_cells {
            bail!("{cells} railed cells, expected at most {max_cells}");
        }
        Ok(())
    }

    fn railed_cells(&self) -> usize {
        self.bytes
            .chunks_exact(SIM_BYTES as usize)
            .filter(|chunk| {
                let h = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]).abs();
                let v = f32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]).abs();
                h >= 47.0 || v >= 1410.0
            })
            .count()
    }

    fn assert_quiet(&self, x: u32, y: u32, limit: f32) -> Result<()> {
        let at = ((y * self.width + x) * SIM_BYTES) as usize;
        for channel in 0..2 {
            let at = at + channel * 4;
            let value = f32::from_le_bytes([
                self.bytes[at],
                self.bytes[at + 1],
                self.bytes[at + 2],
                self.bytes[at + 3],
            ]);
            if value.abs() > limit {
                bail!(
                    "poisoned cell survived as {} in channel {channel}, limit {limit}",
                    value.abs(),
                );
            }
        }
        Ok(())
    }

    fn assert_stirred(&self, floor: f32) -> Result<()> {
        let peak = self
            .bytes
            .chunks_exact(SIM_BYTES as usize)
            .flat_map(|cell| [channel(cell, 0).abs(), channel(cell, 1).abs()])
            .fold(0.0_f32, f32::max);
        if peak <= floor {
            bail!("basin peak {peak} did not exceed {floor}");
        }
        Ok(())
    }

    fn assert_quiet_everywhere(&self, ceiling: f32) -> Result<()> {
        let peak = self
            .bytes
            .chunks_exact(SIM_BYTES as usize)
            .flat_map(|cell| [channel(cell, 0).abs(), channel(cell, 1).abs()])
            .fold(0.0_f32, f32::max);
        if peak > ceiling {
            bail!("quiet basin peak {peak} exceeded {ceiling}");
        }
        Ok(())
    }
}

fn channel(cell: &[u8], channel: usize) -> f32 {
    let at = channel * 4;
    f32::from_le_bytes([cell[at], cell[at + 1], cell[at + 2], cell[at + 3]])
}

fn quiet(tide: f32) -> Frame {
    quiet_for(super::super::surface::SurfaceId(1), tide)
}

fn quiet_for(surface: super::super::surface::SurfaceId, tide: f32) -> Frame {
    Frame {
        surface,
        life: Arc::new(()),
        generation: 0,
        dry: false,
        veil: None,
        tensions: Vec::new(),
        lifts: Vec::new(),
        domain: Domain {
            rect: water_rect(),
            enclosed: 0.0,
        },
        scroll_tilt: 0.0,
        splashes: Vec::new(),
        raft: None,
        floor: Floor::NONE,
        viewer: far_rect(),
        touches: Vec::new(),
        wake: true,
        tide,
        chemistry: Chemistry::default(),
        guard: true,
    }
}

struct Script {
    tensions: Vec<Tension>,
    lifts: Vec<Lift>,
    splashes: Vec<Splash>,
    raft: Option<Raft>,
}

impl Script {
    fn storm(frame: usize) -> Self {
        let mut tensions = Vec::with_capacity(QUIVER_SLOTS);
        let mut lifts = Vec::with_capacity(LIFT_SLOTS);
        let mut splashes = Vec::with_capacity(SPLASH_SLOTS);
        for i in 0..QUIVER_SLOTS {
            let x = 120.0 + ((frame * 37 + i * 83) % 460) as f32;
            let y = 24.0 + ((frame * 29 + i * 71) % 300) as f32;
            let rect = egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(44.0, 20.0));
            tensions.push(Tension {
                rect,
                pointer: rect.center(),
                grip: 0.35 + i as f32 * 0.18,
                omega: if i % 2 == 0 { 0.0 } else { 0.3 * TAU },
            });
        }
        for i in 0..LIFT_SLOTS {
            let x = 104.0 + ((frame * 53 + i * 97) % 430) as f32;
            let y = 18.0 + ((frame * 47 + i * 67) % 270) as f32;
            let rect = egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(92.0, 118.0));
            lifts.push(if i == 3 {
                Lift::shallow(rect, 0.78)
            } else {
                Lift::surface(rect, 0.35 + i as f32 * 0.19)
            });
        }
        for i in 0..SPLASH_SLOTS {
            let x = 96.0 + ((frame * 23 + i * 31) % 500) as f32;
            let y = ((frame * 19 + i * 43) % 330) as f32;
            let rect = egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(72.0, 96.0));
            splashes.push(Splash {
                rect,
                age: (i as f32 % 11.0) * 0.023,
                amp: 10.0 + i as f32 % 7.0,
                shape: if i % 4 == 0 {
                    SplashShape::Slide
                } else {
                    SplashShape::Ring
                },
                travel: if i.is_multiple_of(2) { 1.0 } else { -1.0 },
            });
        }
        let raft = frame.is_multiple_of(3).then(|| {
            let rect = egui::Rect::from_min_size(egui::pos2(240.0, 96.0), egui::vec2(180.0, 96.0));
            Raft {
                rect,
                corners: [
                    1.0 + (frame as f32 * 0.07).sin() * 4.0,
                    2.0 + (frame as f32 * 0.11).cos() * 3.5,
                    3.0 + (frame as f32 * 0.13).sin() * 3.0,
                    1.5 + (frame as f32 * 0.17).cos() * 4.5,
                ],
            }
        });
        Self {
            tensions,
            lifts,
            splashes,
            raft,
        }
    }

    fn spaz(frame: usize) -> Self {
        let mut splashes = Vec::with_capacity(SPLASH_SLOTS);
        let rect = egui::Rect::from_min_size(egui::pos2(260.0, 120.0), egui::vec2(104.0, 132.0));
        for i in 0..SPLASH_SLOTS {
            let jitter = egui::vec2(
                ((i * 19 + frame * 7) % 21) as f32 - 10.0,
                ((i * 23 + frame * 11) % 21) as f32 - 10.0,
            );
            splashes.push(Splash {
                rect: rect.translate(jitter),
                age: ((i + frame) % 8) as f32 * 0.006,
                amp: 1.8,
                shape: if i % 4 == 0 {
                    SplashShape::Slide
                } else {
                    SplashShape::Ring
                },
                // exercise the directional dipole path under load too
                travel: [0.0, 1.0, -1.0][i % 3],
            });
        }
        Self {
            tensions: Vec::new(),
            lifts: vec![Lift::surface(rect, 1.0)],
            splashes,
            raft: None,
        }
    }

    fn frame(&self, tide: f32) -> Frame {
        Frame {
            surface: super::super::surface::SurfaceId(1),
            life: Arc::new(()),
            generation: 0,
            dry: false,
            veil: None,
            tensions: self.tensions.clone(),
            lifts: self.lifts.clone(),
            domain: Domain {
                rect: water_rect(),
                enclosed: 0.0,
            },
            scroll_tilt: ((tide * 2.3).sin() * 14.0).clamp(-18.0, 18.0),
            splashes: self.splashes.clone(),
            raft: self.raft,
            floor: Floor::NONE,
            viewer: far_rect(),
            touches: Vec::new(),
            wake: true,
            tide,
            chemistry: Chemistry::default(),
            guard: true,
        }
    }

    fn very_wet_frame(&self, tide: f32) -> Frame {
        let mut chemistry = Chemistry::default();
        chemistry.wave_damp *= 2.0;
        chemistry.height_retention = 1.0 - (1.0 - chemistry.height_retention) / 2.0;
        Frame {
            surface: super::super::surface::SurfaceId(1),
            life: Arc::new(()),
            generation: 0,
            dry: false,
            veil: None,
            tensions: self.tensions.clone(),
            lifts: self.lifts.clone(),
            domain: Domain {
                rect: water_rect(),
                enclosed: 0.0,
            },
            scroll_tilt: 0.0,
            splashes: self.splashes.clone(),
            raft: self.raft,
            floor: Floor::NONE,
            viewer: far_rect(),
            touches: Vec::new(),
            wake: true,
            tide,
            chemistry,
            guard: true,
        }
    }
}

fn water_rect() -> egui::Rect {
    egui::Rect::from_min_max(egui::pos2(96.0, 0.0), egui::pos2(W as f32, H as f32))
}

fn far_rect() -> egui::Rect {
    egui::Rect::from_min_size(egui::pos2(-4e6, -4e6), egui::Vec2::ZERO)
}

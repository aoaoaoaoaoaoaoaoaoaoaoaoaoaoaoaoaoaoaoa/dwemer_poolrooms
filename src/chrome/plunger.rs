//! Shared mechanics and baked-foundry transport for controls that move normal
//! to the faceplate.

use std::{collections::HashMap, sync::Arc, time::Duration};

use egui::{Color32, Pos2, Rect, Stroke, Vec2};

use super::{ForgedMesh, ForgedVertex, HOT, foundry};

const INTEGRATOR_STEP: f32 = 1.0 / 240.0;
const SYNTHETIC_PRESS_TIME: f32 = 0.065;

pub(super) type BakedVertex = ForgedVertex;
pub(super) type BakedMesh = ForgedMesh;

#[derive(Clone, Copy)]
pub(super) struct BakedShadow {
    pub(super) mesh: BakedMesh,
}

#[derive(Clone, Copy)]
pub(super) struct BakedPose {
    pub(super) elevation: f32,
    pub(super) button: BakedMesh,
    pub(super) shadow: BakedMesh,
}

#[derive(Clone, Copy)]
pub(super) struct BakedGauge {
    pub(super) side: u8,
    pub(super) socket_half: f32,
    pub(super) top_half: f32,
    pub(super) body_half: f32,
    pub(super) socket: BakedMesh,
    pub(super) poses: &'static [BakedPose],
}

#[derive(Clone, Copy)]
pub(super) struct BakedGuard {
    pub(super) mesh: BakedMesh,
    pub(super) floor_shadow: BakedMesh,
    pub(super) crown_shadow: BakedShadow,
}

#[derive(Clone, Default)]
pub(super) struct GuardCache {
    origin: Option<Pos2>,
    atlas: Option<usize>,
    mesh: Option<Arc<egui::Mesh>>,
    floor_shadow: Option<Arc<egui::Mesh>>,
    crown_shadows: HashMap<usize, Arc<egui::Mesh>>,
}

impl GuardCache {
    pub(super) fn prepare(
        &mut self,
        origin: Pos2,
        atlas: usize,
        guard: BakedGuard,
        pose: usize,
        receiver_z: f32,
        eye_z: f32,
        slope: f32,
        guarded: bool,
    ) -> RenderedGuard {
        if self.origin != Some(origin) || self.atlas != Some(atlas) {
            *self = Self {
                origin: Some(origin),
                atlas: Some(atlas),
                ..Self::default()
            };
        }
        if !guarded {
            return RenderedGuard::default();
        }
        let mesh = self
            .mesh
            .get_or_insert_with(|| instantiate(guard.mesh, origin))
            .clone();
        let floor_shadow = self
            .floor_shadow
            .get_or_insert_with(|| instantiate(guard.floor_shadow, origin))
            .clone();
        let crown_shadow = self
            .crown_shadows
            .entry(pose)
            .or_insert_with(|| {
                instantiate_shadow(guard.crown_shadow, origin, receiver_z, eye_z, slope)
            })
            .clone();
        RenderedGuard {
            mesh: Some(mesh),
            floor_shadow: Some(floor_shadow),
            crown_shadow: Some(crown_shadow),
        }
    }
}

#[derive(Clone, Default)]
pub(super) struct RenderedGuard {
    mesh: Option<Arc<egui::Mesh>>,
    floor_shadow: Option<Arc<egui::Mesh>>,
    crown_shadow: Option<Arc<egui::Mesh>>,
}

impl RenderedGuard {
    pub(super) fn paint_floor(&self, painter: &egui::Painter, clip: Rect) {
        if let Some(shadow) = &self.floor_shadow {
            foundry::paint_compiled(painter, clip, shadow);
        }
    }

    pub(super) fn paint_crown(&self, painter: &egui::Painter, clip: Rect) {
        if let Some(shadow) = &self.crown_shadow {
            foundry::paint_compiled(painter, clip, shadow);
        }
        if let Some(mesh) = &self.mesh {
            foundry::paint_compiled(painter, clip, mesh);
        }
    }

    #[cfg(test)]
    pub(super) fn installed(&self) -> bool {
        self.mesh.is_some() && self.floor_shadow.is_some() && self.crown_shadow.is_some()
    }
}

#[derive(Clone, Copy)]
pub(super) struct SpringLaw {
    pub(super) stiffness: f32,
    pub(super) damping: f32,
    pub(super) restitution: f32,
    pub(super) floor: f32,
    pub(super) ceiling: f32,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Spring {
    position: f32,
    velocity: f32,
}

impl Spring {
    pub(super) const fn at(position: f32) -> Self {
        Self {
            position,
            velocity: 0.0,
        }
    }

    fn strike(&mut self, velocity: f32) {
        if velocity.is_sign_negative() {
            self.velocity = self.velocity.min(velocity);
        } else {
            self.velocity = self.velocity.max(velocity);
        }
    }

    pub(super) fn advance(&mut self, target: f32, dt: f32, law: SpringLaw) {
        let steps = (dt / INTEGRATOR_STEP).ceil() as u32;
        let h = dt / steps.max(1) as f32;
        for _ in 0..steps {
            self.velocity +=
                (-law.stiffness * (self.position - target) - law.damping * self.velocity) * h;
            self.position += self.velocity * h;
            if self.position < law.floor {
                self.position = law.floor;
                self.velocity = self.velocity.abs() * law.restitution;
            } else if self.position > law.ceiling {
                self.position = law.ceiling;
                self.velocity = -self.velocity.abs() * law.restitution;
            }
        }
    }

    fn moving(self, target: f32) -> bool {
        (self.position - target).abs() > 0.001 || self.velocity.abs() > 0.01
    }
}

#[derive(Clone, Copy)]
pub(super) struct Motion {
    pub(super) position: f32,
    pub(super) travel: f32,
}

/// Drive the shared stiff-spring law for a momentary foundry plunger.
pub(super) fn momentary_motion(
    ui: &egui::Ui,
    response: &egui::Response,
    enabled: bool,
    activated: bool,
    rest: f32,
    press: f32,
    law: SpringLaw,
) -> Motion {
    let dt = ui
        .input(|input| input.stable_dt)
        .clamp(1.0 / 240.0, 1.0 / 30.0);
    let synthetic = enabled && activated && !response.clicked_by(egui::PointerButton::Primary);
    // Egui assigns click ownership after the press frame. Observe that frame
    // directly so a sleeping host sees the first stroke before button-up.
    let struck_here = response.hovered()
        && ui.input(|input| input.pointer.primary_pressed() && input.pointer.primary_down());
    let held = enabled
        && (response.is_pointer_button_down_on()
            || struck_here
            || synthetic_pressure(ui, response.id, synthetic, dt));
    motion(
        ui,
        response.id,
        rest,
        if held { press } else { rest },
        (struck_here || synthetic).then_some(-54.0),
        dt,
        law,
    )
}

/// Drive the shared stiff-spring law between two stable latches and one
/// pointer-pressure overtravel stop.
pub(super) fn latching_motion(
    ui: &egui::Ui,
    response: &egui::Response,
    enabled: bool,
    activated: bool,
    latched: bool,
    raised: f32,
    seated: f32,
    press: f32,
    strike: f32,
    law: SpringLaw,
) -> Motion {
    let target = if enabled && response.is_pointer_button_down_on() {
        press
    } else if latched {
        seated
    } else {
        raised
    };
    let dt = ui
        .input(|input| input.stable_dt)
        .clamp(1.0 / 240.0, 1.0 / 30.0);
    motion(
        ui,
        response.id,
        if latched { seated } else { raised },
        target,
        activated.then_some(strike),
        dt,
        law,
    )
}

fn synthetic_pressure(ui: &egui::Ui, id: egui::Id, fire: bool, dt: f32) -> bool {
    let held = ui.ctx().data_mut(|data| {
        let key = id.with("synthetic-pressure");
        let mut remaining = data.get_temp::<f32>(key).unwrap_or(0.0);
        if fire {
            remaining = SYNTHETIC_PRESS_TIME;
        }
        let held = remaining > 0.0;
        remaining = (remaining - dt).max(0.0);
        let _old = data.insert_temp(key, remaining);
        held
    });
    if held {
        ui.ctx().request_repaint();
    }
    held
}

pub(super) fn motion(
    ui: &egui::Ui,
    id: egui::Id,
    seed: f32,
    target: f32,
    strike: Option<f32>,
    dt: f32,
    law: SpringLaw,
) -> Motion {
    let key = id.with("foundry-spring");
    let (motion, moving) = ui.ctx().data_mut(|data| {
        let mut spring = data
            .get_temp::<Spring>(key)
            .unwrap_or_else(|| Spring::at(seed));
        let before = spring.position;
        if let Some(velocity) = strike {
            spring.strike(velocity);
        }
        spring.advance(target, dt, law);
        let moving = spring.moving(target);
        let motion = Motion {
            position: spring.position,
            travel: spring.position - before,
        };
        let _old = data.insert_temp(key, spring);
        (motion, moving)
    });
    if moving {
        // A delayed wake cannot be coalesced into the redraw currently being
        // served. This matters to event loops that sleep between input events:
        // an immediate request made from inside RedrawRequested may otherwise
        // vanish, freezing a held mechanism until button-up.
        ui.ctx().request_repaint_after(Duration::from_millis(4));
    }
    motion
}

pub(super) fn pose_index(elevation: f32, min: f32, max: f32, count: usize) -> usize {
    let t = ((elevation - min) / (max - min)).clamp(0.0, 1.0);
    ((t * (count - 1) as f32).round() as usize).min(count - 1)
}

pub(super) fn instantiate(baked: BakedMesh, origin: Pos2) -> Arc<egui::Mesh> {
    let mut mesh = egui::Mesh::default();
    mesh.vertices.reserve(baked.vertices.len());
    mesh.indices.reserve(baked.indices.len());
    for vertex in baked.vertices {
        let [x, y] = vertex.position;
        let [r, g, b, a] = vertex.color;
        mesh.colored_vertex(
            origin + Vec2::new(x, y),
            Color32::from_rgba_unmultiplied(r, g, b, a),
        );
    }
    mesh.indices.extend_from_slice(baked.indices);
    Arc::new(mesh)
}

#[derive(Clone)]
struct InstalledPose {
    button: Arc<egui::Mesh>,
    shadow: Arc<egui::Mesh>,
}

#[derive(Clone, Default)]
struct PoseCache {
    origin: Option<Pos2>,
    atlas: usize,
    socket: Option<Arc<egui::Mesh>>,
    poses: HashMap<usize, InstalledPose>,
}

impl PoseCache {
    fn prepare(
        &mut self,
        origin: Pos2,
        atlas: usize,
        pose_index: usize,
        socket: BakedMesh,
        poses: &'static [BakedPose],
    ) -> (Arc<egui::Mesh>, InstalledPose) {
        if self.origin != Some(origin) || self.atlas != atlas {
            *self = Self {
                origin: Some(origin),
                atlas,
                socket: Some(instantiate(socket, origin)),
                ..Self::default()
            };
        }
        let socket = self
            .socket
            .get_or_insert_with(|| instantiate(socket, origin))
            .clone();
        let pose = self
            .poses
            .entry(pose_index)
            .or_insert_with(|| {
                let pose = poses[pose_index];
                InstalledPose {
                    button: instantiate(pose.button, origin),
                    shadow: instantiate(pose.shadow, origin),
                }
            })
            .clone();
        (socket, pose)
    }
}

#[derive(Clone, Copy)]
pub(super) struct MomentaryAnatomy {
    pub(super) assembly: Rect,
    pub(super) socket: Rect,
    pub(super) button: Rect,
}

impl MomentaryAnatomy {
    pub(super) fn new(
        rect: Rect,
        side: f32,
        socket_half: f32,
        body_half: f32,
        pixels_per_point: f32,
    ) -> Self {
        debug_assert!(pixels_per_point.is_finite() && pixels_per_point > 0.0);
        let casing_side = 2.0 * (socket_half - foundry::law::MOMENTARY_CASING_INSET);
        let physical_side = (casing_side * pixels_per_point).round() as u32;
        // Even physical spans center on a pixel boundary; odd spans center on
        // one pixel. Fixing that phase once keeps every nested die coaxial.
        let phase = if physical_side.is_multiple_of(2) {
            0.0
        } else {
            0.5
        };
        let snap = |coordinate: f32| {
            ((coordinate * pixels_per_point - phase).round() + phase) / pixels_per_point
        };
        let center = rect.center();
        let center = Pos2::new(snap(center.x), snap(center.y));
        let assembly = Rect::from_center_size(center, Vec2::splat(side));
        Self {
            socket: Rect::from_center_size(assembly.center(), Vec2::splat(socket_half * 2.0)),
            button: Rect::from_center_size(assembly.center(), Vec2::splat(body_half * 2.0)),
            assembly,
        }
    }
}

pub(super) fn paint_momentary(
    ui: &egui::Ui,
    painter: &egui::Painter,
    anatomy: MomentaryAnatomy,
    elevation: f32,
    id: egui::Id,
    focused: bool,
    atlas: usize,
    socket: BakedMesh,
    poses: &'static [BakedPose],
    pose_min: f32,
    pose_max: f32,
    paint_crown: impl FnOnce(&egui::Painter, Rect, Pos2),
) {
    let origin = anatomy.socket.center();
    let pose_index = pose_index(elevation, pose_min, pose_max, poses.len());
    let (socket, rendered) = ui.ctx().data_mut(|data| {
        data.get_temp_mut_or_default::<PoseCache>(id.with("compiled-foundry"))
            .prepare(origin, atlas, pose_index, socket, poses)
    });

    foundry::socket_void(painter, anatomy.socket);
    foundry::paint_compiled(painter, anatomy.socket.shrink(1.0), &rendered.shadow);
    let aperture = anatomy.socket.shrink(foundry::RIM_WIDTH);
    foundry::paint_compiled(painter, aperture, &rendered.button);
    paint_crown(painter, aperture, origin);
    foundry::paint_compiled(painter, anatomy.socket, &socket);

    if focused {
        let _focus = painter.rect_stroke(
            anatomy.assembly.shrink(0.5),
            1.0,
            Stroke::new(1.0_f32, HOT.gamma_multiply(0.44)),
            egui::StrokeKind::Inside,
        );
    }
}

pub(super) fn instantiate_shadow(
    shadow: BakedShadow,
    origin: Pos2,
    receiver_z: f32,
    eye_z: f32,
    slope: f32,
) -> Arc<egui::Mesh> {
    let scale = eye_z / (eye_z - receiver_z);
    let mut mesh = egui::Mesh::default();
    mesh.vertices.reserve(shadow.mesh.vertices.len());
    mesh.indices.reserve(shadow.mesh.indices.len());
    for vertex in shadow.mesh.vertices {
        let [x, y_plus_slope_z] = vertex.position;
        let [r, g, b, a] = vertex.color;
        mesh.colored_vertex(
            origin + Vec2::new(x * scale, (y_plus_slope_z - slope * receiver_z) * scale),
            Color32::from_rgba_unmultiplied(r, g, b, a),
        );
    }
    mesh.indices.extend_from_slice(shadow.mesh.indices);
    Arc::new(mesh)
}

/// Signed volume swept by a faceplate-normal plunger.
#[derive(Clone, Copy, Debug)]
pub struct PlungerWake {
    rect: Rect,
    travel: f32,
    volume: f32,
}

impl PlungerWake {
    pub(super) fn new(rect: Rect, travel: f32) -> Option<Self> {
        (travel.abs() >= 0.002).then_some(Self {
            rect,
            travel,
            volume: rect.area() * travel.abs(),
        })
    }

    /// Screen-space footprint occupied by the moving solid.
    pub fn rect(self) -> Rect {
        self.rect
    }

    /// Signed travel normal to the faceplate. Positive is toward the viewer.
    pub fn travel(self) -> f32 {
        self.travel
    }

    /// Absolute swept volume in logical point³.
    pub fn swept_volume(self) -> f32 {
        self.volume
    }
}

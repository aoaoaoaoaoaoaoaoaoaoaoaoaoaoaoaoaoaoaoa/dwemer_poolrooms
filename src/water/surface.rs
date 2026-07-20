use std::{
    hash::Hash,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use super::{
    engine,
    machines::{EmptyDrain, LoadingRaft, scale_rect},
};

const SOURCE_LIFE: f32 = 0.24;
const TOOLTIP_GRIP: f32 = 0.72;
const WATER_WAKE: Duration = Duration::from_secs(14);
const QUIVER_WAKE: Duration = Duration::from_secs(8);
const QUIVER_EPSILON: f32 = 0.012;
const TILT_TELEPORT: f32 = 2500.0;
const TILT_SPEED_CEIL: f32 = 14_000.0;
const FORCE_CEIL: f32 = 48.0;
const FORCE_EPSILON: f32 = 0.015;
static NEXT_SURFACE_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) struct SurfaceId(pub(super) u64);

impl SurfaceId {
    fn mint() -> Self {
        Self(NEXT_SURFACE_ID.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Wetness {
    Dry,
    #[default]
    Wet,
    Deluge,
}

#[derive(Clone, Copy)]
struct Drench {
    wave: f32,
    glyph: f32,
    optics: f32,
    decay: f32,
}

impl Wetness {
    fn drench(self) -> Drench {
        match self {
            Self::Dry => Drench {
                wave: 0.0,
                glyph: 0.0,
                optics: 0.0,
                decay: 1.0,
            },
            Self::Wet => Drench {
                wave: 1.25,
                glyph: 0.75,
                optics: 1.0,
                decay: 1.0,
            },
            Self::Deluge => Drench {
                wave: 2.0,
                glyph: 2.0,
                optics: 2.0,
                decay: 2.0,
            },
        }
    }
}

/// CPU-side contact law. Shader chemistry remains in [`super::Chemistry`].
#[derive(Clone, Copy, Debug)]
pub struct Agitation {
    pub enter_impulse: f32,
    pub exit_impulse: f32,
    pub click_impulse: f32,
    pub thwack_impulse: f32,
    pub text_impulse: f32,
    pub pond_impulse: f32,
    pub pond_life: f32,
    pub quiver_release: f32,
    pub scroll_coupling: f32,
    pub scroll_memory: f32,
    pub poison_sweep: bool,
    pub lift_rise: f32,
    pub lift_fall: f32,
}

impl Default for Agitation {
    fn default() -> Self {
        Self {
            enter_impulse: 0.9,
            exit_impulse: 0.42,
            click_impulse: 2.5,
            thwack_impulse: 0.24,
            text_impulse: 1.02,
            pond_impulse: 1.6,
            pond_life: 8.0,
            quiver_release: 0.48,
            scroll_coupling: 0.02,
            scroll_memory: 0.11,
            poison_sweep: true,
            lift_rise: 0.09,
            lift_fall: 0.24,
        }
    }
}

/// A freely placed excitation. Amplitudes are canonical and are scaled by the
/// surface's current wetness; ages, capacity, and GPU representation stay private.
#[derive(Clone, Copy, Debug)]
pub enum Poke {
    Ring { impulse: f32 },
    Basin { impulse: f32 },
    Drag { impulse: f32, travel: f32 },
    Jitter { impulse: f32 },
}

impl Poke {
    pub const fn ring(impulse: f32) -> Self {
        Self::Ring { impulse }
    }
    pub const fn basin(impulse: f32) -> Self {
        Self::Basin { impulse }
    }
    pub const fn drag(impulse: f32, travel: f32) -> Self {
        Self::Drag { impulse, travel }
    }
    pub const fn jitter(impulse: f32) -> Self {
        Self::Jitter { impulse }
    }

    fn vitals(self) -> (f32, engine::SplashShape, f32) {
        match self {
            Self::Ring { impulse } => (impulse, engine::SplashShape::Ring, 0.0),
            Self::Basin { impulse } => (impulse, engine::SplashShape::Basin, 0.0),
            Self::Drag { impulse, travel } => (impulse, engine::SplashShape::Ring, travel),
            Self::Jitter { impulse } => (impulse, engine::SplashShape::Jitter, 0.0),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Cut {
    rect: egui::Rect,
    radius: f32,
    barrier: bool,
}

impl Cut {
    pub const NONE: Self = Self {
        rect: egui::Rect {
            min: egui::Pos2 { x: -4e6, y: -4e6 },
            max: egui::Pos2 { x: -4e6, y: -4e6 },
        },
        radius: 0.0,
        barrier: false,
    };

    /// A sharp optical aperture occupied by solid foreground geometry.
    pub const fn barrier(rect: egui::Rect, radius: f32) -> Self {
        Self {
            rect,
            radius,
            barrier: true,
        }
    }

    /// A sharp optical aperture through which the active basin remains visible.
    pub const fn aperture(rect: egui::Rect, radius: f32) -> Self {
        Self {
            rect,
            radius,
            barrier: false,
        }
    }

    fn physical(self, scale: f32) -> engine::Cut {
        engine::Cut {
            rect: scale_rect(self.rect, scale),
            radius: self.radius * scale,
            barrier: if self.barrier { 1.0 } else { 0.0 },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DomainKind {
    Shelf,
    Basin,
}

/// The physical extent and boundary law of one water world.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Domain {
    rect: egui::Rect,
    kind: DomainKind,
}

/// A visible substrate beneath the water. Depth controls optical extinction,
/// not solver geometry: both modes receive the same refractive field.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Floor {
    rect: egui::Rect,
    depth: f32,
}

impl Floor {
    /// Lamplit tile near the surface, used while the gallery is empty.
    pub const fn shallow(rect: egui::Rect) -> Self {
        Self { rect, depth: 0.0 }
    }

    /// The same tile sunk into darker water, suitable for enclosed basins.
    pub const fn deep(rect: egui::Rect) -> Self {
        Self { rect, depth: 0.68 }
    }

    fn physical(self, scale: f32) -> engine::Floor {
        engine::Floor {
            rect: scale_rect(self.rect, scale),
            depth: self.depth,
        }
    }
}

impl Domain {
    /// Deep water beginning at `rect.min.x`, with transmitted shallows to its
    /// left and the viewport edges as the outer walls.
    pub const fn shelf(rect: egui::Rect) -> Self {
        Self {
            rect,
            kind: DomainKind::Shelf,
        }
    }

    /// A closed rectangular basin whose four edges reflect the field.
    pub const fn basin(rect: egui::Rect) -> Self {
        Self {
            rect,
            kind: DomainKind::Basin,
        }
    }

    fn physical(self, scale: f32) -> engine::Domain {
        engine::Domain {
            rect: scale_rect(self.rect, scale),
            enclosed: if self.kind == DomainKind::Basin {
                1.0
            } else {
                0.0
            },
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Veil {
    pub cuts: [Cut; 2],
    pub strength: f32,
    pub dim: f32,
    pub blur: f32,
}

impl Veil {
    fn physical(self, scale: f32) -> engine::Veil {
        engine::Veil {
            cuts: self.cuts.map(|cut| cut.physical(scale)),
            strength: self.strength,
            dim: self.dim,
            blur: self.blur,
        }
    }
}

struct LiftPlate {
    id: u64,
    rect: egui::Rect,
    grip: f32,
}

struct Plunge {
    rect: egui::Rect,
    born: Instant,
    amp: f32,
    shape: engine::SplashShape,
    drag: f32,
}

struct TouchPlunge {
    center: egui::Pos2,
    born: Instant,
    amp: f32,
}

#[derive(Clone, Copy)]
struct Quiver {
    id: u64,
    rect: egui::Rect,
    pointer: egui::Pos2,
    grip: f32,
    omega: f32,
}

impl Quiver {
    fn physical(self, scale: f32) -> engine::Tension {
        engine::Tension {
            rect: scale_rect(self.rect, scale),
            pointer: (self.pointer.to_vec2() * scale).to_pos2(),
            grip: self.grip,
            omega: self.omega,
        }
    }
}

#[derive(Default)]
enum TrayTilt {
    #[default]
    Virgin,
    Awake {
        offset: f32,
        velocity: f32,
    },
}

impl TrayTilt {
    fn sway(&mut self, offset: f32, scale: f32, dt: f32, coupling: f32, tau: f32) -> f32 {
        let Self::Awake {
            offset: last,
            velocity,
        } = self
        else {
            *self = Self::Awake {
                offset,
                velocity: 0.0,
            };
            return 0.0;
        };
        let delta = (offset - *last) * scale;
        *last = offset;
        if delta.abs() > TILT_TELEPORT {
            *velocity = 0.0;
            return 0.0;
        }
        let sample = (delta / dt).clamp(-TILT_SPEED_CEIL, TILT_SPEED_CEIL);
        let alpha = 1.0 - (-dt / tau.max(0.02)).exp();
        *velocity += (sample - *velocity) * alpha;
        *velocity = velocity.clamp(-TILT_SPEED_CEIL, TILT_SPEED_CEIL);
        if velocity.abs() < FORCE_EPSILON / coupling.max(1.0e-6) && sample.abs() < 1.0 {
            *velocity = 0.0;
        }
        (*velocity * coupling).clamp(-FORCE_CEIL, FORCE_CEIL)
    }
}

/// One application-facing water world. It owns forcing history, oscillators,
/// lifetimes, scaling, wake policy, and the identity of its persistent basin.
pub struct Surface {
    id: SurfaceId,
    life: Arc<()>,
    generation: u64,
    wetness: Wetness,
    chemistry: engine::Chemistry,
    agitation: Agitation,
    domain: Domain,
    floor: Option<Floor>,
    active_lift: Option<(u64, egui::Rect)>,
    lift_plates: Vec<LiftPlate>,
    lift_memo: Option<(u64, egui::Rect)>,
    plunges: Vec<Plunge>,
    pond_open: bool,
    pond: egui::Rect,
    touches: Vec<TouchPlunge>,
    loading: LoadingRaft,
    drain: EmptyDrain,
    scroll: TrayTilt,
    scroll_tilt: f32,
    water_until: Option<Instant>,
    quivers: Vec<Quiver>,
    quiver_tick: Instant,
    quiver_until: Option<Instant>,
    epoch: Instant,
}

impl Default for Surface {
    fn default() -> Self {
        Self::new(Wetness::Wet)
    }
}

impl Surface {
    pub fn new(wetness: Wetness) -> Self {
        let now = Instant::now();
        Self {
            id: SurfaceId::mint(),
            life: Arc::new(()),
            generation: 0,
            wetness,
            chemistry: engine::Chemistry::default(),
            agitation: Agitation::default(),
            domain: Domain::shelf(egui::Rect::ZERO),
            floor: None,
            active_lift: None,
            lift_plates: Vec::new(),
            lift_memo: None,
            plunges: Vec::new(),
            pond_open: false,
            pond: far_rect(),
            touches: Vec::new(),
            loading: LoadingRaft::new(),
            drain: EmptyDrain::new(),
            scroll: TrayTilt::default(),
            scroll_tilt: 0.0,
            water_until: None,
            quivers: Vec::new(),
            quiver_tick: now,
            quiver_until: None,
            epoch: now,
        }
    }

    pub fn wetness(&self) -> Wetness {
        self.wetness
    }
    pub fn set_wetness(&mut self, wetness: Wetness) {
        self.wetness = wetness;
    }
    pub fn chemistry(&self) -> &super::Chemistry {
        &self.chemistry
    }
    pub fn chemistry_mut(&mut self) -> &mut super::Chemistry {
        &mut self.chemistry
    }
    pub fn agitation(&self) -> &Agitation {
        &self.agitation
    }
    pub fn agitation_mut(&mut self) -> &mut Agitation {
        &mut self.agitation
    }

    /// Borrow both halves of the live calibration surface without exposing
    /// its temporal machinery.
    pub fn laboratory_mut(&mut self) -> (&mut super::Chemistry, &mut Agitation) {
        (&mut self.chemistry, &mut self.agitation)
    }

    pub fn reset_laboratory(&mut self) {
        self.chemistry = super::Chemistry::default();
        self.agitation = Agitation::default();
    }

    pub fn domain(&self) -> egui::Rect {
        self.domain.rect
    }

    /// Start one UI pass and declare this world's physical domain.
    pub fn begin(&mut self, domain: Domain) {
        self.domain = domain;
        self.active_lift = None;
    }

    /// Erase this world's CPU forcing history and its persistent GPU basin on
    /// the next composition without disturbing its calibrated laws.
    pub fn reset(&mut self) {
        let now = Instant::now();
        self.generation = self.generation.wrapping_add(1);
        self.domain = Domain::shelf(egui::Rect::ZERO);
        self.floor = None;
        self.active_lift = None;
        self.lift_plates.clear();
        self.lift_memo = None;
        self.plunges.clear();
        self.pond_open = false;
        self.pond = far_rect();
        self.touches.clear();
        self.loading.hide();
        self.drain.hide();
        self.scroll = TrayTilt::default();
        self.scroll_tilt = 0.0;
        self.water_until = None;
        self.quivers.clear();
        self.quiver_tick = now;
        self.quiver_until = None;
        self.epoch = now;
    }

    pub fn hover(&mut self, id: impl Hash, rect: egui::Rect) {
        self.active_lift = Some((egui::Id::new(id).value(), rect));
    }

    pub fn set_floor(&mut self, floor: Option<Floor>) {
        self.floor = floor;
    }

    /// Start one UI pass for an optional bounded pond. Touches outside an open
    /// pond are retired; its wall owns the reflected ripple law.
    pub fn begin_pond(&mut self, open: bool) {
        self.pond_open = open;
        self.pond = far_rect();
        if !open {
            self.touches.clear();
        }
    }

    pub fn pond_surface(&mut self, rect: egui::Rect) {
        self.pond = rect;
    }

    pub fn close_pond(&mut self) {
        self.pond_open = false;
        self.pond = far_rect();
        self.touches.clear();
    }

    pub fn touch(&mut self, center: egui::Pos2) {
        if self.touches.len() >= engine::TOUCH_SLOTS {
            let _oldest = self.touches.remove(0);
        }
        self.touches.push(TouchPlunge {
            center,
            born: Instant::now(),
            amp: self.agitation.pond_impulse * self.wetness.drench().wave,
        });
        self.arm();
    }

    /// Place an excitation anywhere on the surface in logical egui coordinates.
    pub fn poke(&mut self, rect: egui::Rect, poke: Poke) {
        let (amp, shape, drag) = poke.vitals();
        self.poke_scaled(rect, amp * self.wetness.drench().wave, shape, drag);
    }

    pub fn bump(&mut self, rect: egui::Rect) {
        self.poke(rect, Poke::ring(0.18));
    }
    pub fn click(&mut self, rect: egui::Rect) {
        self.poke(rect, Poke::ring(self.agitation.click_impulse));
    }
    pub fn lever(&mut self, rect: egui::Rect, sign: f32) {
        self.poke(rect, Poke::basin(sign * 0.5));
    }
    pub fn drag(&mut self, rect: egui::Rect, travel: f32) {
        self.poke(rect, Poke::drag(0.16, travel));
    }
    pub fn select(&mut self, rect: egui::Rect) {
        self.poke(rect, Poke::ring(0.45));
    }
    pub fn thwack(&mut self, rect: egui::Rect, energy: f32) {
        self.poke(rect, Poke::jitter(self.agitation.thwack_impulse * energy));
    }
    pub fn fold(&mut self, wake: Option<crate::chrome::FoldWake>) {
        let Some(wake) = wake else {
            return;
        };
        let amp = match wake.flux {
            crate::chrome::FoldFlux::Open => -0.72,
            crate::chrome::FoldFlux::Close => 0.92,
        };
        self.poke(wake.rect, Poke::basin(amp));
    }
    pub fn text(&mut self, wake: crate::chrome::TextWake) {
        let raw = wake.amp(self.agitation.text_impulse).clamp(0.25, 6.6);
        if raw >= 0.25 {
            self.poke_scaled(
                wake.rect,
                raw * self.wetness.drench().glyph,
                engine::SplashShape::Ring,
                0.0,
            );
        }
    }

    pub fn heave(&mut self, ctx: &egui::Context, offset: f32) {
        if !self.domain.rect.is_positive() {
            self.scroll_tilt = 0.0;
            return;
        }
        let dt = ctx.input(|input| input.stable_dt).clamp(1.0 / 240.0, 0.08);
        self.scroll_tilt = self.scroll.sway(
            offset,
            ctx.pixels_per_point(),
            dt,
            self.agitation.scroll_coupling * self.wetness.drench().wave,
            self.agitation.scroll_memory,
        );
        if self.scroll_tilt.abs() > 0.015 {
            self.arm();
            ctx.request_repaint();
        }
    }

    pub fn show_loading(&mut self, ctx: &egui::Context, rect: egui::Rect) {
        if self.wetness != Wetness::Dry {
            self.loading.show(ctx, rect);
            self.arm();
        }
    }
    pub fn hide_loading(&mut self) {
        self.loading.hide();
    }
    pub fn show_drain(&mut self, ctx: &egui::Context, rect: egui::Rect) {
        if self.wetness == Wetness::Dry {
            return;
        }
        for pulse in self.drain.show(ctx, rect) {
            self.poke(pulse.rect, Poke::basin(pulse.amp));
        }
    }
    pub fn hide_drain(&mut self) {
        self.drain.hide();
    }

    /// Seal all UI forcing into one opaque physical-pixel frame for the renderer.
    pub fn frame(
        &mut self,
        ctx: &egui::Context,
        pixels_per_point: f32,
        tooltip_rects: &[egui::Rect],
        veil: Option<Veil>,
    ) -> Frame {
        self.settle_lifts(ctx);
        let lifts = self.physical_lifts(pixels_per_point, tooltip_rects);
        self.plunges
            .retain(|p| p.born.elapsed().as_secs_f32() <= SOURCE_LIFE);
        if !self.plunges.is_empty() {
            ctx.request_repaint();
        }
        let splashes = self
            .plunges
            .iter()
            .map(|p| engine::Splash {
                rect: scale_rect(p.rect, pixels_per_point),
                age: p.born.elapsed().as_secs_f32(),
                amp: p.amp,
                shape: p.shape,
                drag: p.drag,
            })
            .collect();
        let raft = self
            .loading
            .source(ctx, pixels_per_point, self.wetness.drench().wave);
        let life = self.agitation.pond_life * self.wetness.drench().decay;
        self.touches
            .retain(|t| self.pond_open && retire(t.born.elapsed().as_secs_f32(), life) > 0.0);
        if !self.touches.is_empty() {
            ctx.request_repaint();
        }
        let touches = self
            .touches
            .iter()
            .map(|t| {
                let age = t.born.elapsed().as_secs_f32();
                engine::Touch {
                    center: (t.center.to_vec2() * pixels_per_point).to_pos2(),
                    age,
                    amp: t.amp * retire(age, life),
                }
            })
            .collect();
        let tensions = take_tensions(
            ctx,
            pixels_per_point,
            &mut self.quivers,
            &mut self.quiver_tick,
            self.agitation.quiver_release,
        );
        if !tensions.is_empty() {
            self.quiver_until = Some(Instant::now() + QUIVER_WAKE);
        }
        let now = Instant::now();
        let quiver_wake = self.quiver_until.is_some_and(|until| until > now);
        if !quiver_wake {
            self.quiver_until = None;
        }
        let water_wake = self.water_until.is_some_and(|until| until > now);
        if !water_wake {
            self.water_until = None;
        }
        let repaint = water_wake || quiver_wake;
        if repaint {
            ctx.request_repaint();
        }
        let domain = if self.domain.rect.is_positive() {
            self.domain
        } else {
            Domain::shelf(ctx.content_rect())
        };
        Frame {
            surface: self.id,
            life: Arc::clone(&self.life),
            generation: self.generation,
            dry: self.wetness == Wetness::Dry,
            veil: veil.map(|v| v.physical(pixels_per_point)),
            tensions,
            lifts,
            domain: domain.physical(pixels_per_point),
            scroll_tilt: self.scroll_tilt,
            splashes,
            raft,
            floor: self.floor.map_or(engine::Floor::NONE, |floor| {
                floor.physical(pixels_per_point)
            }),
            viewer: scale_rect(self.pond, pixels_per_point),
            touches,
            wake: repaint,
            tide: self.epoch.elapsed().as_secs_f32() % 1000.0,
            chemistry: scaled_chemistry(self.chemistry, self.wetness),
            guard: self.agitation.poison_sweep,
        }
    }

    fn poke_scaled(&mut self, rect: egui::Rect, amp: f32, shape: engine::SplashShape, drag: f32) {
        if amp.abs() <= f32::EPSILON {
            return;
        }
        if self.plunges.len() >= engine::SPLASH_SLOTS {
            let victim = self
                .plunges
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| a.amp.abs().total_cmp(&b.amp.abs()))
                .map_or(0, |(slot, _)| slot);
            let _weakest = self.plunges.remove(victim);
        }
        self.plunges.push(Plunge {
            rect,
            born: Instant::now(),
            amp,
            shape,
            drag,
        });
        self.arm();
    }

    fn arm(&mut self) {
        self.water_until = Some(Instant::now() + WATER_WAKE);
    }

    fn settle_lifts(&mut self, ctx: &egui::Context) {
        let active_id = self.active_lift.map(|(id, _)| id);
        if active_id != self.lift_memo.map(|(id, _)| id) {
            if let Some((_, rect)) = self.lift_memo {
                self.poke(rect, Poke::ring(self.agitation.exit_impulse));
            }
            if let Some((_, rect)) = self.active_lift {
                self.poke(rect, Poke::ring(self.agitation.enter_impulse));
            }
            self.lift_memo = self.active_lift;
        }
        if let Some((id, rect)) = self.active_lift {
            match self.lift_plates.iter_mut().find(|plate| plate.id == id) {
                Some(plate) => plate.rect = rect,
                None => self.lift_plates.push(LiftPlate {
                    id,
                    rect,
                    grip: 0.0,
                }),
            }
        }
        let dt = ctx.input(|input| input.stable_dt).clamp(0.0, 0.1);
        let mut animating = false;
        for plate in &mut self.lift_plates {
            let target = f32::from(active_id == Some(plate.id));
            let tau = if target > plate.grip {
                self.agitation.lift_rise
            } else {
                self.agitation.lift_fall
            };
            plate.grip += (target - plate.grip) * (1.0 - (-dt / tau).exp());
            animating |= (plate.grip - target).abs() > 0.002;
        }
        self.lift_plates
            .retain(|plate| plate.grip > 0.002 || active_id == Some(plate.id));
        if self.lift_plates.len() > engine::LIFT_SLOTS {
            self.lift_plates.sort_by(|a, b| b.grip.total_cmp(&a.grip));
            self.lift_plates.truncate(engine::LIFT_SLOTS);
        }
        if animating {
            ctx.request_repaint();
        }
    }

    fn physical_lifts(&self, scale: f32, tooltip_rects: &[egui::Rect]) -> Vec<engine::Lift> {
        let tooltip_slots = tooltip_rects.len().min(1);
        let image_slots = engine::LIFT_SLOTS.saturating_sub(tooltip_slots);
        let mut plates = self
            .lift_plates
            .iter()
            .filter(|p| p.grip > 0.0)
            .collect::<Vec<_>>();
        plates.sort_by(|a, b| b.grip.total_cmp(&a.grip));
        let mut lifts = plates
            .into_iter()
            .take(image_slots)
            .map(|p| engine::Lift::surface(scale_rect(p.rect, scale), p.grip))
            .collect::<Vec<_>>();
        lifts.extend(
            tooltip_rects
                .iter()
                .take(tooltip_slots)
                .copied()
                .map(|rect| engine::Lift::shallow(scale_rect(rect, scale), TOOLTIP_GRIP)),
        );
        lifts
    }
}

/// Opaque, renderer-ready snapshot. Consumers can inspect scheduling only;
/// shader capacity and packing are deliberately inaccessible.
pub struct Frame {
    pub(super) surface: SurfaceId,
    pub(super) life: Arc<()>,
    pub(super) generation: u64,
    pub(super) dry: bool,
    pub(super) veil: Option<engine::Veil>,
    pub(super) tensions: Vec<engine::Tension>,
    pub(super) lifts: Vec<engine::Lift>,
    pub(super) domain: engine::Domain,
    pub(super) scroll_tilt: f32,
    pub(super) splashes: Vec<engine::Splash>,
    pub(super) raft: Option<engine::Raft>,
    pub(super) floor: engine::Floor,
    pub(super) viewer: egui::Rect,
    pub(super) touches: Vec<engine::Touch>,
    pub(super) wake: bool,
    pub(super) tide: f32,
    pub(super) chemistry: engine::Chemistry,
    pub(super) guard: bool,
}

impl Frame {
    pub fn live(&self) -> bool {
        !self.dry
    }
    pub fn dry(&self) -> bool {
        self.dry
    }
    pub fn wants_repaint(&self) -> bool {
        self.wake
    }

    pub(super) fn sim_live(&self) -> bool {
        !self.dry
            && (!self.tensions.is_empty()
                || !self.lifts.is_empty()
                || !self.splashes.is_empty()
                || self.raft.is_some()
                || self.wake)
    }
}

fn take_tensions(
    ctx: &egui::Context,
    scale: f32,
    bank: &mut Vec<Quiver>,
    then: &mut Instant,
    release: f32,
) -> Vec<engine::Tension> {
    let now = Instant::now();
    let dt = now.duration_since(*then).as_secs_f32().clamp(0.0, 0.12);
    *then = now;
    for q in bank.iter_mut() {
        q.grip *= (-dt / release.max(0.03)).exp();
    }
    for seed in crate::tide::take(ctx) {
        let incoming = Quiver {
            id: seed.id,
            rect: seed.rect,
            pointer: seed.pointer,
            grip: seed.grip,
            omega: seed.omega,
        };
        match bank.iter_mut().find(|q| q.id == incoming.id) {
            Some(q) => {
                q.rect = incoming.rect;
                q.pointer = incoming.pointer;
                q.grip = q.grip.max(incoming.grip);
                q.omega = incoming.omega;
            }
            None => bank.push(incoming),
        }
    }
    bank.retain(|q| q.grip > QUIVER_EPSILON);
    bank.sort_by(|a, b| b.grip.total_cmp(&a.grip));
    bank.iter()
        .take(engine::QUIVER_SLOTS)
        .copied()
        .map(|q| q.physical(scale))
        .collect()
}

fn scaled_chemistry(mut chemistry: engine::Chemistry, wetness: Wetness) -> engine::Chemistry {
    let drench = wetness.drench();
    chemistry.refract_px *= drench.optics;
    chemistry.ior_spread *= drench.optics;
    chemistry.meniscus_px *= drench.wave;
    chemistry.tremor_amp *= drench.wave;
    chemistry.wave_damp *= drench.decay;
    chemistry.height_retention = 1.0 - (1.0 - chemistry.height_retention) / drench.decay;
    chemistry
}

fn retire(age: f32, life: f32) -> f32 {
    let t = ((age - life) / 2.5).clamp(0.0, 1.0);
    1.0 - t * t * (3.0 - 2.0 * t)
}

fn far_rect() -> egui::Rect {
    egui::Rect::from_min_size(egui::pos2(-4e6, -4e6), egui::Vec2::ZERO)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_scroll_frame_hits_force_ceiling_without_second_lowpass() {
        let mut tray = TrayTilt::default();
        let _virgin = tray.sway(0.0, 1.0, 1.0 / 60.0, 0.08, 0.11);
        let force = tray.sway(100.0, 1.0, 1.0 / 60.0, 0.08, 0.11);
        assert!(force > 40.0, "force was {force}");
    }

    #[test]
    fn arbitrary_pokes_hide_shader_capacity() {
        let mut surface = Surface::default();
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(10.0, 10.0));
        for i in 0..(engine::SPLASH_SLOTS * 3) {
            surface.poke(rect, Poke::ring(i as f32 + 1.0));
        }
        assert_eq!(surface.plunges.len(), engine::SPLASH_SLOTS);
        assert!(
            surface
                .plunges
                .iter()
                .all(|p| p.amp >= (engine::SPLASH_SLOTS * 2) as f32)
        );
    }

    #[test]
    fn wetness_scales_chemistry_without_mutating_laboratory_values() {
        let surface = Surface::new(Wetness::Deluge);
        let scaled = scaled_chemistry(*surface.chemistry(), surface.wetness());
        assert_eq!(
            surface.chemistry().refract_px,
            engine::Chemistry::default().refract_px
        );
        assert_eq!(scaled.refract_px, surface.chemistry().refract_px * 2.0);
        assert!((scaled.tremor_omega - 0.9 * std::f32::consts::TAU).abs() < 1e-5);
    }

    #[test]
    fn wetness_preserves_the_shipped_calibration() {
        let wet = Wetness::Wet.drench();
        assert_eq!(
            (wet.wave, wet.glyph, wet.optics, wet.decay),
            (1.25, 0.75, 1.0, 1.0)
        );
        let deluge = Wetness::Deluge.drench();
        assert_eq!(
            (deluge.wave, deluge.glyph, deluge.optics, deluge.decay),
            (2.0, 2.0, 2.0, 2.0)
        );
    }
}

//! A square plunger whose one engraved glyph is its entire label. It may return
//! after activation or remain at a lower boolean latch.
//! Crown, bevel, skirt, projection, illumination, and directional shadow are
//! compiled from a three-dimensional foundry model; runtime selects a pose and
//! integrates the stiff return spring.

#![deny(missing_docs)]

use std::ops::Deref;

use egui::{
    Atom, Button, CursorIcon, FontId, Pos2, Rect, Response, Sense, Vec2, WidgetInfo, WidgetType,
};

use super::{MechanismSize, Symbol, foundry};

use super::mechanism::{CouplingPorts, CouplingTarget, sealed};
use super::plunger::{
    self, BakedGuard, BakedMesh, BakedPose, BakedShadow, BakedVertex, GuardCache, PlungerWake,
    SpringLaw,
};

const ETCH_EM_PER_CROWN: f32 = 13.5 / (8.9 * 2.0);
const BRIGHT_CUT_DEPTH: f32 = 0.72;
const FLAT_CUT_DEPTH: f32 = 0.96;
const SPRING_LAW: SpringLaw = SpringLaw {
    stiffness: 2_400.0,
    damping: 68.0,
    restitution: 0.12,
    floor: baked::POSE_MIN,
    ceiling: baked::POSE_MAX,
};

#[derive(Clone, Copy)]
struct BakedMonoglyphGauge {
    side: u8,
    socket_half: f32,
    top_half: f32,
    body_half: f32,
    guard: BakedGuard,
    socket: BakedMesh,
    poses: &'static [BakedPose],
}

mod baked {
    use super::{BakedGuard, BakedMesh, BakedMonoglyphGauge, BakedPose, BakedShadow, BakedVertex};

    include!(concat!(env!("OUT_DIR"), "/monoglyph_atlas.rs"));
}

/// Material and cutter treatment applied to a monoglyph's mark.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MonoglyphFinish {
    /// A shallow action cut dominated by its illuminated fresh-bronze wall.
    #[default]
    BrightCut,
    /// A steep, flat-bottomed engraving whose floor is soot black.
    Void,
    /// A steep, flat-bottomed engraving filled with rough blood-ochre paint.
    Danger,
    /// A steep, flat-bottomed engraving filled with rough deep-pink paint.
    Love,
}

impl MonoglyphFinish {
    /// Complete finish register in stable gallery order.
    pub const ALL: [Self; 4] = [Self::BrightCut, Self::Void, Self::Danger, Self::Love];

    /// Stable material name for galleries and instrumentation.
    pub const fn name(self) -> &'static str {
        match self {
            Self::BrightCut => "BRIGHT CUT",
            Self::Void => "VOID",
            Self::Danger => "DANGER",
            Self::Love => "LOVE",
        }
    }

    const fn depth(self) -> f32 {
        match self {
            Self::BrightCut => BRIGHT_CUT_DEPTH,
            Self::Void | Self::Danger | Self::Love => FLAT_CUT_DEPTH,
        }
    }

    fn exposure(self, elevation: f32) -> f32 {
        let shade = foundry::law::monoglyph_shade(elevation);
        match self {
            Self::BrightCut => shade.bright_cut,
            Self::Void => shade.void,
            Self::Danger => shade.danger,
            Self::Love => shade.love,
        }
    }
}

/// A square Poolrooms button carrying exactly one engraved glyph.
///
/// The `char` constructor makes the one-glyph boundary structural: text labels
/// and rectangular actions cannot accidentally enter this mechanism.
/// [`Monoglyph::symbol`] selects common action marks from the typed Poolrooms
/// armory so their Unicode scalar and S/M/L typography cannot drift between
/// applications. Pointer pressure plunges the flat crown into its black
/// socket; release excites a stiff underdamped spring that makes one small
/// return bounce.
/// [`Monoglyph::show_latched`] binds the same mechanism to a boolean state: the
/// selected state rests at a lower latch while pointer pressure retains a
/// deeper overtravel stroke.
/// A disabled mechanism retains its live mark and crown position beneath a
/// fixed-stock protective grille.
/// [`Monoglyph::size`] selects one of the exact gauges admitted by
/// [`MechanismSize`].
///
/// # Example
///
/// ```
/// use brass_poolrooms::{chrome::{Monoglyph, Symbol}, egui};
///
/// fn decrement(ui: &mut egui::Ui) -> bool {
///     Monoglyph::symbol(Symbol::Decrement).show(ui).clicked()
/// }
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Monoglyph {
    glyph: char,
    size: MechanismSize,
    finish: MonoglyphFinish,
    symbol: Option<Symbol>,
    focusable: bool,
}

impl Monoglyph {
    /// Forge a momentary button around one Unicode scalar.
    pub const fn new(glyph: char) -> Self {
        Self {
            glyph,
            size: MechanismSize::Large,
            finish: MonoglyphFinish::BrightCut,
            symbol: None,
            focusable: true,
        }
    }

    /// Forge one canonical action mark from the shared symbology armory.
    ///
    /// The selected [`MechanismSize`] remains the sole typographic gauge:
    /// equal symbols at equal sizes therefore have identical glyph, font,
    /// crown, relief, and motion. The symbol's semantic finish default is
    /// selected from the armory's closed lookup table.
    pub const fn symbol(symbol: Symbol) -> Self {
        Self {
            glyph: symbol.glyph(),
            size: MechanismSize::Large,
            finish: symbol.default_finish(),
            symbol: Some(symbol),
            focusable: true,
        }
    }

    /// Select a build-time forged square footprint.
    pub const fn size(mut self, size: MechanismSize) -> Self {
        self.size = size;
        self
    }

    /// Override the raw-glyph or semantic-symbol finish.
    ///
    /// This is deliberately applied after [`Monoglyph::symbol`] resolves the
    /// armory default, so a destructive symbol may be rendered in another
    /// material when its local meaning demands it.
    pub const fn finish(mut self, finish: MonoglyphFinish) -> Self {
        self.finish = finish;
        self
    }

    /// Include or omit this actuator from keyboard focus traversal.
    ///
    /// Pointer activation remains available when focus is omitted. This is
    /// intended for redundant boundary actuators whose command already has a
    /// keyboard binding and whose presence must not perturb application
    /// navigation.
    pub const fn focusable(mut self, focusable: bool) -> Self {
        self.focusable = focusable;
        self
    }

    /// Lay out, actuate, and paint the complete square mechanism.
    ///
    /// The response dereferences to [`egui::Response`] and carries the signed
    /// volume swept by the button during this frame. Pass it to
    /// `water::Surface::monoglyph` during the same UI pass to couple the plunge
    /// and return stroke into the active water world.
    pub fn show(self, ui: &mut egui::Ui) -> MonoglyphResponse {
        self.show_with_latch(ui, None)
    }

    /// Lay out and actuate the mechanism as a two-state latching button.
    ///
    /// Activation mutates `latched` immediately, marks the ordinary egui
    /// response as changed, and reports checkbox accessibility semantics. The
    /// selected crown remains visibly seated until a later activation releases
    /// it. Water coupling is identical to [`Monoglyph::show`].
    pub fn show_latched(self, ui: &mut egui::Ui, latched: &mut bool) -> MonoglyphResponse {
        self.show_with_latch(ui, Some(latched))
    }

    fn show_with_latch(self, ui: &mut egui::Ui, latch: Option<&mut bool>) -> MonoglyphResponse {
        let (atlas, gauge) = self.gauge();
        let sense = if self.focusable {
            Sense::click()
        } else {
            Sense::CLICK
        };
        let (rect, mut response) = ui.allocate_exact_size(Vec2::splat(self.size.side()), sense);
        let enabled = ui.is_enabled();
        if enabled {
            response = response.on_hover_cursor(CursorIcon::PointingHand);
        }
        let activated = super::exact_activation(ui, &response);
        let label = self
            .symbol
            .map_or_else(|| self.glyph.to_string(), |symbol| symbol.name().to_owned());
        let motion = if let Some(latched) = latch {
            if activated {
                *latched = !*latched;
                response.mark_changed();
            }
            response.widget_info(|| {
                WidgetInfo::selected(WidgetType::Checkbox, enabled, *latched, label.clone())
            });
            plunger::latching_motion(
                ui,
                &response,
                enabled,
                activated,
                *latched,
                baked::REST,
                baked::LATCH,
                baked::PRESS,
                -42.0 * self.size.side() / MechanismSize::Large.side(),
                SPRING_LAW,
            )
        } else {
            response
                .widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, label.clone()));
            plunger::momentary_motion(
                ui,
                &response,
                enabled,
                activated,
                baked::REST,
                baked::PRESS,
                SPRING_LAW,
            )
        };
        let anatomy = plunger::MomentaryAnatomy::new(
            rect,
            self.size.side(),
            gauge.socket_half,
            gauge.body_half,
            ui.pixels_per_point(),
        );
        let pose = plunger::pose_index(
            motion.position,
            baked::POSE_MIN,
            baked::POSE_MAX,
            gauge.poses.len(),
        );
        let guard = ui.ctx().data_mut(|data| {
            data.get_temp_mut_or_default::<GuardCache>(response.id.with("compiled-guard"))
                .prepare(
                    anatomy.socket.center(),
                    atlas,
                    gauge.guard,
                    pose,
                    gauge.poses[pose].elevation,
                    baked::SHADOW_EYE_Z,
                    baked::SHADOW_SLOPE,
                    !enabled,
                )
        });
        let mut painter = ui.painter().clone();
        if !enabled {
            painter.set_opacity(1.0);
        }
        guard.paint_floor(&painter, anatomy.assembly.expand(2.0));
        plunger::paint_momentary(
            ui,
            &painter,
            anatomy,
            motion.position,
            response.id,
            response.has_focus(),
            atlas,
            gauge.socket,
            gauge.poses,
            baked::POSE_MIN,
            baked::POSE_MAX,
            |painter, aperture, origin| {
                etch(
                    painter,
                    aperture,
                    origin,
                    self.glyph,
                    self.finish,
                    motion.position,
                    gauge.top_half,
                );
            },
        );
        guard.paint_crown(&painter, anatomy.assembly.expand(2.0));
        super::tension(ui, &response);

        MonoglyphResponse {
            wake: MonoglyphWake::new(anatomy.button, motion.travel),
            response,
            elevation: motion.position,
            ports: CouplingPorts::around(anatomy.socket),
            activated,
        }
    }

    /// Embed the mechanism's resting pose as an inert legend inside `button`.
    ///
    /// The monoglyph remains part of the enclosing button's allocation and
    /// interaction surface; it does not introduce a nested actuator or focus
    /// stop. The monoglyph terminates the parent plate: its casing covers the
    /// trailing frame and consumes that frame's redundant inset. The gap to
    /// the command label equals the vertical frame inset.
    pub fn show_in(self, ui: &mut egui::Ui, button: Button<'_>) -> Response {
        let (atlas, gauge) = self.gauge();
        let side = self.size.side();
        let padding = ui.spacing().button_padding;
        let atom_size = Vec2::new(side - padding.x, side);
        debug_assert!(atom_size.x > 0.0);
        let id = ui.next_auto_id().with("inline-monoglyph");
        let layout = button
            .small()
            .right_text(Atom::custom(id, atom_size))
            .gap(padding.y)
            .atom_ui(ui);
        if let Some(atom) = layout.rect(id) {
            let rect = Rect::from_min_size(atom.left_top(), Vec2::splat(side));
            let anatomy = plunger::MomentaryAnatomy::new(
                rect,
                side,
                gauge.socket_half,
                gauge.body_half,
                ui.pixels_per_point(),
            );
            let guard = ui.ctx().data_mut(|data| {
                let pose = plunger::pose_index(
                    baked::REST,
                    baked::POSE_MIN,
                    baked::POSE_MAX,
                    gauge.poses.len(),
                );
                data.get_temp_mut_or_default::<GuardCache>(id.with("compiled-guard"))
                    .prepare(
                        anatomy.socket.center(),
                        atlas,
                        gauge.guard,
                        pose,
                        gauge.poses[pose].elevation,
                        baked::SHADOW_EYE_Z,
                        baked::SHADOW_SLOPE,
                        !ui.is_enabled(),
                    )
            });
            let mut painter = ui.painter().clone();
            if !ui.is_enabled() {
                painter.set_opacity(1.0);
            }
            guard.paint_floor(&painter, anatomy.assembly.expand(2.0));
            plunger::paint_momentary(
                ui,
                &painter,
                anatomy,
                baked::REST,
                id,
                false,
                atlas,
                gauge.socket,
                gauge.poses,
                baked::POSE_MIN,
                baked::POSE_MAX,
                |painter, aperture, origin| {
                    etch(
                        painter,
                        aperture,
                        origin,
                        self.glyph,
                        self.finish,
                        baked::REST,
                        gauge.top_half,
                    );
                },
            );
            guard.paint_crown(&painter, anatomy.assembly.expand(2.0));
        }
        layout.response
    }

    fn gauge(self) -> (usize, BakedMonoglyphGauge) {
        let atlas = self.size.atlas_index();
        let gauge = baked::GAUGES[atlas];
        let law = foundry::law::momentary_gauge(gauge.side);
        debug_assert_eq!(gauge.side, self.size.side() as u8);
        debug_assert_eq!(gauge.socket_half, law.socket_half);
        debug_assert_eq!(gauge.top_half, law.top_half);
        debug_assert_eq!(gauge.body_half, law.body_half);
        (atlas, gauge)
    }
}

#[must_use = "the response carries both egui state and displaced-water volume"]
/// Interaction state and displaced-water geometry from one [`Monoglyph`] frame.
pub struct MonoglyphResponse {
    response: Response,
    wake: Option<MonoglyphWake>,
    elevation: f32,
    ports: CouplingPorts,
    activated: bool,
}

impl MonoglyphResponse {
    /// Whether pointer, accessibility, or exact keyboard activation fired it.
    pub const fn clicked(&self) -> bool {
        self.activated
    }

    /// The plunger volume swept since the preceding frame, if it moved.
    pub fn wake(&self) -> Option<MonoglyphWake> {
        self.wake
    }

    /// Current crown elevation normal to the faceplate, in logical points.
    pub fn elevation(&self) -> f32 {
        self.elevation
    }

    /// Attach a tooltip while retaining the mechanism's physical response.
    pub fn on_hover_text(mut self, text: impl Into<egui::WidgetText>) -> Self {
        self.response = self.response.on_hover_text(text);
        self
    }

    /// Discard physical displacement and return the ordinary egui response.
    pub fn into_response(self) -> Response {
        self.response
    }
}

impl Deref for MonoglyphResponse {
    type Target = Response;

    fn deref(&self) -> &Self::Target {
        &self.response
    }
}

impl sealed::Sealed for MonoglyphResponse {}

impl CouplingTarget for MonoglyphResponse {
    fn coupling_ports(&self) -> CouplingPorts {
        self.ports
    }
}

/// Signed swept volume from a monoglyph plunger.
pub type MonoglyphWake = PlungerWake;

fn etch(
    painter: &egui::Painter,
    clip: Rect,
    origin: Pos2,
    glyph: char,
    finish: MonoglyphFinish,
    elevation: f32,
    top_half: f32,
) {
    let depth = finish.depth();
    let exposure = finish.exposure(elevation);
    let floor_scale = foundry::perspective_scale(elevation - depth);
    let font = FontId::monospace(top_half * 2.0 * ETCH_EM_PER_CROWN * floor_scale);
    let galley = painter.layout_no_wrap(glyph.to_string(), font, egui::Color32::PLACEHOLDER);
    let pos = origin - galley.mesh_bounds.center().to_vec2();
    match finish {
        MonoglyphFinish::BrightCut => {
            foundry::bright_cut_etch(painter, clip, pos, galley, elevation, depth, exposure);
        }
        MonoglyphFinish::Void => {
            foundry::flat_cut_etch(
                painter,
                clip,
                pos,
                galley,
                elevation,
                depth,
                foundry::EngravingFloor::Void,
                exposure,
            );
        }
        MonoglyphFinish::Danger => {
            foundry::flat_cut_etch(
                painter,
                clip,
                pos,
                galley,
                elevation,
                depth,
                foundry::EngravingFloor::Danger(glyph as u32),
                exposure,
            );
        }
        MonoglyphFinish::Love => {
            foundry::flat_cut_etch(
                painter,
                clip,
                pos,
                galley,
                elevation,
                depth,
                foundry::EngravingFloor::Love(glyph as u32),
                exposure,
            );
        }
    }
}

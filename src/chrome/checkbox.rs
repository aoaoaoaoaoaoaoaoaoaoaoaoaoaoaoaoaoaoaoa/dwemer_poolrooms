//! A latching square plunger under a sprung protective grille. The plunger's
//! two boolean states are literal z-stops: unchecked stands proud, checked
//! seats down in its aperture. Pointer pressure drives it below either latch;
//! release changes the latch and a stiff underdamped spring closes the motion.
//!
//! Disabled controls do not change their state indication. A welded-wire cage
//! occupies the hand volume above the mechanism while leaving its elevation
//! visible. Crown, skirt, wire, welds, and frame are physical triangle meshes;
//! the build-time foundry compiler performs projection, visibility,
//! illumination, and directional shadow casting once, then runtime replays its
//! 2D vector pose atlas.
//!
//! Optional descriptions inhabit a casing-height bronze plaque with 45° edge
//! facets and two cylindrical ties. Its parameterized plate geometry and
//! dynamic flat-bottomed text cut are projected under the same camera and
//! illuminant.

#![deny(missing_docs)]

use std::{collections::HashMap, ops::Deref, sync::Arc};

use egui::{
    CursorIcon, Pos2, Rect, Sense, Stroke, TextStyle, TextWrapMode, Vec2, WidgetInfo, WidgetText,
    WidgetType,
};

use super::{COUPLING_SPACING, HOT, foundry};

use super::mechanism::{CouplingPorts, CouplingTarget, MechanismSize, sealed};
use super::plunger::{
    self, BakedGuard, BakedMesh, BakedPose, BakedShadow, BakedVertex, GuardCache, PlungerWake,
    SpringLaw,
};

#[derive(Clone, Copy)]
struct BakedCheckboxGauge {
    side: u8,
    control_height: f32,
    assembly_side: f32,
    socket_half: f32,
    body_half: f32,
    latch_up: f32,
    latch_down: f32,
    pose_min: f32,
    pose_max: f32,
    wire_count: u8,
    guard: BakedGuard,
    poses: &'static [BakedPose],
}

fn spring_law(gauge: BakedCheckboxGauge) -> SpringLaw {
    SpringLaw {
        stiffness: 1_700.0,
        damping: 42.0,
        restitution: 0.16,
        floor: gauge.pose_min,
        ceiling: gauge.pose_max,
    }
}

mod baked {
    use super::{BakedCheckboxGauge, BakedGuard, BakedMesh, BakedPose, BakedShadow, BakedVertex};

    include!(concat!(env!("OUT_DIR"), "/checkbox_atlas.rs"));
}

/// A mechanically latching Poolrooms boolean control.
///
/// The unchecked crown stands proud of its aperture; the checked crown rests
/// on the lower latch. Pressing drives either state toward a shared overtravel
/// stop, and release excites the spring around the newly selected latch.
/// A nonempty label is cut into a casing-height bronze plaque joined to the
/// mechanism by two cylindrical ties. [`Checkbox::label_side`] places that
/// plaque on either side. Disabling the surrounding `egui::Ui` installs the
/// physical wire guard while preserving the state geometry and foundry
/// luminance beneath it; the guard, rather than egui's conventional opacity
/// fade, is the disabled affordance. [`Checkbox::size`] selects an independent
/// build-time forge. Compact guards retain the large guard's wire, frame, and
/// weld stock, removing lattice lines instead of shrinking them into
/// alias-prone filaments.
///
/// # Example
///
/// ```
/// use brass_poolrooms::{chrome::{Checkbox, LabelSide}, egui};
///
/// fn controls(ui: &mut egui::Ui, armed: &mut bool) {
///     let checkbox = Checkbox::new(armed, "ARM PUMPS")
///         .label_side(LabelSide::Left)
///         .show(ui);
///     if checkbox.changed() {
///         // `armed` changed latch.
///     }
/// }
/// ```
pub struct Checkbox<'a> {
    checked: &'a mut bool,
    label: Option<WidgetText>,
    label_side: LabelSide,
    size: MechanismSize,
}

/// Side of a mechanism occupied by its etched identification plaque.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LabelSide {
    /// Place the plaque to the left of the mechanism.
    Left,
    /// Place the plaque to the right of the mechanism.
    #[default]
    Right,
}

impl<'a> Checkbox<'a> {
    /// Construct a latching plunger with a right-hand etched plaque.
    ///
    /// An empty label elides the plaque and its ties entirely.
    pub fn new(checked: &'a mut bool, label: impl Into<WidgetText>) -> Self {
        Self {
            checked,
            label: Some(label.into()),
            label_side: LabelSide::Right,
            size: MechanismSize::Large,
        }
    }

    /// Construct an unlabelled latching plunger.
    pub fn without_text(checked: &'a mut bool) -> Self {
        Self {
            checked,
            label: None,
            label_side: LabelSide::Right,
            size: MechanismSize::Large,
        }
    }

    /// Place the etched plaque to the left or right of the plunger casing.
    ///
    /// This has no visible effect on an unlabelled checkbox.
    pub fn label_side(mut self, side: LabelSide) -> Self {
        self.label_side = side;
        self
    }

    /// Select a build-time forged plunger and protective-guard gauge.
    ///
    /// The nominal 20-, 24-, or 32-point gauge governs the plunger. Its
    /// protective guard requires a proportionally larger allocation; fixed
    /// wire stock and progressively coarser lattices keep every size crisp.
    pub const fn size(mut self, size: MechanismSize) -> Self {
        self.size = size;
        self
    }

    /// Lay out, interact with, and paint the complete mechanism.
    ///
    /// The response dereferences to `egui::Response` and carries the signed
    /// volume swept by the plunger during this frame. Pass it to
    /// `water::Surface::checkbox` during the same UI pass to couple that motion
    /// into the active water world.
    pub fn show(self, ui: &mut egui::Ui) -> CheckboxResponse {
        let Self {
            checked,
            label,
            label_side,
            size,
        } = self;
        let atlas = size.atlas_index();
        let gauge = baked::GAUGES[atlas];
        debug_assert_eq!(gauge.side, size.side() as u8);
        debug_assert_eq!(usize::from(gauge.wire_count), atlas + 2);
        let label_text = label.as_ref().map_or("", WidgetText::text).to_owned();
        let plaque = label.and_then(|label| {
            let galley = label.into_galley(
                ui,
                Some(TextWrapMode::Extend),
                f32::INFINITY,
                TextStyle::Button,
            );
            (!galley.is_empty()).then(|| foundry::Plaque::new(galley, gauge.socket_half * 2.0))
        });
        let desired = footprint(gauge, plaque.as_ref().map(foundry::Plaque::size));
        let (rect, mut response) = ui.allocate_exact_size(desired, Sense::click());
        let enabled = ui.is_enabled();
        if enabled {
            response = response.on_hover_cursor(CursorIcon::PointingHand);
        }
        let activated = super::exact_activation(ui, &response);
        if activated {
            *checked = !*checked;
            response.mark_changed();
        }
        response.widget_info(|| {
            WidgetInfo::selected(WidgetType::Checkbox, enabled, *checked, label_text.clone())
        });

        let anatomy = Anatomy::new(
            rect,
            label_side,
            plaque.as_ref().map(foundry::Plaque::size),
            gauge,
        );
        let scale = f32::from(gauge.side) / MechanismSize::Large.side();
        let motion = plunger::latching_motion(
            ui,
            &response,
            enabled,
            activated,
            *checked,
            gauge.latch_up,
            gauge.latch_down,
            gauge.pose_min,
            -32.0 * scale,
            spring_law(gauge),
        );
        let mut painter = ui.painter().clone();
        if !enabled {
            // The grille is the disabled affordance. Egui's inherited opacity
            // would counterfeit a second, nonphysical state change beneath it.
            painter.set_opacity(1.0);
        }
        paint(
            ui,
            &painter,
            anatomy,
            plaque.as_ref(),
            motion.position,
            enabled,
            &response,
            atlas,
            gauge,
        );
        let wake = CheckboxWake::new(anatomy.button, motion.travel);
        CheckboxResponse {
            response,
            wake,
            elevation: motion.position,
            ports: anatomy.coupling_ports(),
            activated,
        }
    }
}

fn footprint(gauge: BakedCheckboxGauge, plaque_size: Option<Vec2>) -> Vec2 {
    plaque_size.map_or(
        Vec2::new(gauge.assembly_side, gauge.control_height),
        |plaque| {
            Vec2::new(
                gauge.assembly_side * 0.5 + gauge.socket_half + COUPLING_SPACING + plaque.x,
                gauge.control_height.max(plaque.y),
            )
        },
    )
}

#[must_use = "the response carries both egui state and displaced-water volume"]
/// Interaction state and displaced-water geometry from one [`Checkbox`] frame.
pub struct CheckboxResponse {
    response: egui::Response,
    wake: Option<CheckboxWake>,
    elevation: f32,
    ports: CouplingPorts,
    activated: bool,
}

impl CheckboxResponse {
    /// Whether pointer, accessibility, or exact keyboard activation toggled it.
    pub const fn clicked(&self) -> bool {
        self.activated
    }

    /// The plunger volume swept since the preceding frame, if it moved.
    pub fn wake(&self) -> Option<CheckboxWake> {
        self.wake
    }

    /// Current crown elevation normal to the faceplate, in logical points.
    /// Positive values stand toward the viewer; negative values lie within
    /// the recess.
    pub fn elevation(&self) -> f32 {
        self.elevation
    }

    /// Discard physical displacement and return the ordinary egui response.
    pub fn into_response(self) -> egui::Response {
        self.response
    }
}

impl Deref for CheckboxResponse {
    type Target = egui::Response;

    fn deref(&self) -> &Self::Target {
        &self.response
    }
}

impl sealed::Sealed for CheckboxResponse {}

impl CouplingTarget for CheckboxResponse {
    fn coupling_ports(&self) -> CouplingPorts {
        self.ports
    }
}

/// Signed swept volume from the checkbox plunger.
pub type CheckboxWake = PlungerWake;

#[derive(Clone, Copy)]
struct Anatomy {
    assembly: Rect,
    socket: Rect,
    button: Rect,
    plaque: Option<Rect>,
}

impl Anatomy {
    fn new(
        rect: Rect,
        side: LabelSide,
        plaque_size: Option<Vec2>,
        gauge: BakedCheckboxGauge,
    ) -> Self {
        let assembly_x = match (side, plaque_size) {
            (_, None) => rect.center().x,
            (LabelSide::Left, Some(_)) => rect.right() - gauge.assembly_side * 0.5,
            (LabelSide::Right, Some(_)) => rect.left() + gauge.assembly_side * 0.5,
        };
        let assembly = Rect::from_center_size(
            Pos2::new(assembly_x, rect.center().y),
            Vec2::splat(gauge.assembly_side),
        );
        let socket =
            Rect::from_center_size(assembly.center(), Vec2::splat(gauge.socket_half * 2.0));
        let button = Rect::from_center_size(assembly.center(), Vec2::splat(gauge.body_half * 2.0));
        let plaque = plaque_size.map(|size| {
            let x = match side {
                LabelSide::Left => socket.left() - COUPLING_SPACING - size.x * 0.5,
                LabelSide::Right => socket.right() + COUPLING_SPACING + size.x * 0.5,
            };
            Rect::from_center_size(Pos2::new(x, rect.center().y), size)
        });
        Self {
            assembly,
            socket,
            button,
            plaque,
        }
    }

    fn coupling_ports(self) -> CouplingPorts {
        match self.plaque {
            Some(plaque) if plaque.center().x < self.socket.center().x => {
                CouplingPorts::spanning(plaque, self.socket)
            }
            Some(plaque) => CouplingPorts::spanning(self.socket, plaque),
            None => CouplingPorts::around(self.socket),
        }
    }
}

fn paint(
    ui: &egui::Ui,
    painter: &egui::Painter,
    anatomy: Anatomy,
    plaque: Option<&foundry::Plaque>,
    elevation: f32,
    enabled: bool,
    response: &egui::Response,
    atlas: usize,
    gauge: BakedCheckboxGauge,
) {
    let origin = anatomy.socket.center();
    let clip = anatomy.assembly.expand(2.0);
    let pose = plunger::pose_index(elevation, gauge.pose_min, gauge.pose_max, baked::POSE_COUNT);
    let rendered = ui.ctx().data_mut(|data| {
        data.get_temp_mut_or_default::<RenderCache>(response.id.with("compiled-foundry"))
            .prepare(origin, atlas, gauge, pose, !enabled)
    });

    if let Some(plaque_rect) = anatomy.plaque {
        let ports = if plaque_rect.center().x < anatomy.socket.center().x {
            (
                CouplingPorts::around(plaque_rect),
                CouplingPorts::around(anatomy.socket),
            )
        } else {
            (
                CouplingPorts::around(anatomy.socket),
                CouplingPorts::around(plaque_rect),
            )
        };
        let _ties = painter.add(foundry::tie_pair(ports.0.right, ports.1.left));
    }
    foundry::socket_bed(painter, anatomy.socket);
    rendered.guard.paint_floor(painter, clip);
    foundry::paint_compiled(painter, anatomy.socket.shrink(1.0), &rendered.button_shadow);
    foundry::paint_compiled(
        painter,
        anatomy.socket.shrink(foundry::RIM_WIDTH),
        &rendered.button,
    );
    foundry::socket_rim(painter, anatomy.socket);

    rendered.guard.paint_crown(painter, clip);
    if let (Some(plaque), Some(rect)) = (plaque, anatomy.plaque) {
        plaque.paint(painter, rect.center());
    }
    if response.has_focus() {
        let _focus = painter.rect_stroke(
            anatomy.assembly.shrink(0.5),
            1.0,
            Stroke::new(1.0_f32, HOT.gamma_multiply(0.44)),
            egui::StrokeKind::Inside,
        );
    }
}

#[derive(Clone)]
struct InstalledPose {
    button: Arc<egui::Mesh>,
    button_shadow: Arc<egui::Mesh>,
}

#[derive(Clone, Default)]
struct RenderCache {
    origin: Option<Pos2>,
    atlas: Option<usize>,
    poses: HashMap<usize, InstalledPose>,
    guard: GuardCache,
}

impl RenderCache {
    fn prepare(
        &mut self,
        origin: Pos2,
        atlas: usize,
        gauge: BakedCheckboxGauge,
        pose_index: usize,
        guarded: bool,
    ) -> Rendered {
        if self.origin != Some(origin) || self.atlas != Some(atlas) {
            *self = Self {
                origin: Some(origin),
                atlas: Some(atlas),
                ..Self::default()
            };
        }
        let pose = gauge.poses[pose_index];
        let installed = self
            .poses
            .entry(pose_index)
            .or_insert_with(|| InstalledPose {
                button: plunger::instantiate(pose.button, origin),
                button_shadow: plunger::instantiate(pose.shadow, origin),
            });
        Rendered {
            button: installed.button.clone(),
            button_shadow: installed.button_shadow.clone(),
            guard: self.guard.prepare(
                origin,
                atlas,
                gauge.guard,
                pose_index,
                pose.elevation,
                baked::SHADOW_EYE_Z,
                baked::SHADOW_SLOPE,
                guarded,
            ),
        }
    }
}

struct Rendered {
    button: Arc<egui::Mesh>,
    button_shadow: Arc<egui::Mesh>,
    guard: plunger::RenderedGuard,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cached_guard_obeys_the_current_enabled_state() {
        let mut cache = RenderCache::default();
        let gauge = baked::GAUGES[MechanismSize::Small.atlas_index()];
        let origin = Pos2::new(20.0, 20.0);

        let guarded = cache.prepare(origin, 0, gauge, 0, true);
        assert!(guarded.guard.installed());

        let enabled = cache.prepare(origin, 0, gauge, 0, false);
        assert!(!enabled.guard.installed());

        let guarded_again = cache.prepare(origin, 0, gauge, 0, true);
        assert!(guarded_again.guard.installed());
    }
}

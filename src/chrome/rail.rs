//! A prismatic carriage driven through an offset slider-crank. The faceplate's
//! guide is the prismatic pair: it fixes carriage attitude while permitting one
//! translation. Beneath it, a crank turns about a fixed deep journal and a
//! connecting rod closes on the carriage. The elbow is the exact intersection
//! of those two rigid-member circles; only that upper rod is visible through
//! the slot.

#![deny(missing_docs)]

use std::{
    num::NonZeroU16,
    ops::{Deref, RangeInclusive},
};

use egui::{Color32, CursorIcon, Key, Modifiers, Pos2, Rect, Sense, Stroke, Vec2, emath::Numeric};

use super::{
    HOT,
    foundry::{self, StockAxis, bronze},
};

const H: f32 = 38.0;
const SLOT_H: f32 = 11.0;
const HANDLE_DIAMETER: f32 = foundry::CONTROL_STOCK_DIAMETER;
const HANDLE_H: f32 = 28.0;
const CLEARANCE: f32 = 1.5;
const DEFAULT_DETENTS: NonZeroU16 = NonZeroU16::MIN.saturating_add(10);

/// A bronze linear control with distinct measuring and admissible spans.
///
/// The `total` span passed to [`Rail::new`] defines the immutable scale and
/// detent positions. [`Rail::allowed`] may narrow the carriage's current travel
/// without changing that scale; forbidden portions are occupied by hatched
/// stop plates instead of disappearing.
///
/// The control supports pointer dragging, clicks, the left/right arrow keys,
/// and Home/End while focused. Its response dereferences to `egui::Response`
/// and also carries the solids' displaced-water wakes.
///
/// # Example
///
/// ```
/// use dwemer_poolrooms::{chrome::Rail, egui};
///
/// fn controls(ui: &mut egui::Ui, value: &mut u16, ceiling: u16) {
///     let rail = Rail::new(value, 0..=10)
///         .allowed(0..=ceiling)
///         .detents(11)
///         .width(320.0)
///         .show(ui);
///
///     if rail.changed() {
///         // `value` now names one of the eleven stations.
///     }
/// }
/// ```
pub struct Rail<'a, N: Numeric> {
    value: &'a mut N,
    total: RangeInclusive<N>,
    allowed: RangeInclusive<N>,
    detents: NonZeroU16,
    width: Option<f32>,
}

impl<'a, N: Numeric> Rail<'a, N> {
    /// Construct a rail whose total and allowed spans initially coincide.
    ///
    /// Small integral spans default to one detent per integer, including both
    /// endpoints. Other spans default to eleven detents; [`Rail::detents`]
    /// overrides either choice.
    pub fn new(value: &'a mut N, total: RangeInclusive<N>) -> Self {
        let allowed = total.clone();
        let lo = total.start().to_f64();
        let hi = total.end().to_f64();
        let integral_span = (hi - lo).round();
        let detents = if N::INTEGRAL && (1.0..=23.0).contains(&integral_span) {
            NonZeroU16::new(integral_span as u16 + 1).unwrap_or(DEFAULT_DETENTS)
        } else {
            DEFAULT_DETENTS
        };
        Self {
            value,
            total,
            allowed,
            detents,
            width: None,
        }
    }

    /// Restrict the carriage without changing the scale or its detent positions.
    ///
    /// The interval must be an ascending, nonempty subset of the total span.
    /// [`Rail::show`] clamps the bound value into it even without interaction.
    pub fn allowed(mut self, allowed: RangeInclusive<N>) -> Self {
        self.allowed = allowed;
        self
    }

    /// Exact number of stable stations, including both endpoints, engraved
    /// into the immutable total span.
    ///
    /// # Panics
    ///
    /// Panics when `count` is less than two.
    pub fn detents(mut self, count: u16) -> Self {
        assert!(count >= 2, "a rail requires at least two detents");
        self.detents = NonZeroU16::new(count).unwrap_or(NonZeroU16::MIN);
        self
    }

    /// Request a width in logical egui points.
    ///
    /// The allocated width is capped by the available UI width and floored at
    /// three handle diameters so the mechanism cannot collapse into itself.
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// Lay out, interact with, and paint the complete rail mechanism.
    ///
    /// The bound value is clamped to the allowed span and snapped to a detent
    /// after pointer interaction. Movement since the preceding frame is
    /// recorded in the returned [`RailResponse`].
    ///
    /// # Panics
    ///
    /// Panics if either span is non-finite or descending, or if the allowed
    /// span lies outside the total span.
    pub fn show(self, ui: &mut egui::Ui) -> RailResponse {
        let Self {
            value,
            total,
            allowed,
            detents,
            width,
        } = self;
        let total = Span::refine(total);
        let allowed = Span::refine(allowed);
        assert!(
            total.lo <= allowed.lo && allowed.hi <= total.hi,
            "rail admissible span must lie inside its total span"
        );

        let before = *value;
        *value = N::from_f64(allowed.clamp(value.to_f64()));
        let width = width.unwrap_or_else(|| ui.available_width());
        let (rect, mut response) = ui.allocate_exact_size(
            Vec2::new(
                width.min(ui.available_width()).max(HANDLE_DIAMETER * 3.0),
                H,
            ),
            Sense::click_and_drag(),
        );
        response = response.on_hover_cursor(CursorIcon::ResizeHorizontal);
        let anatomy = Anatomy::new(rect);

        if response.is_pointer_button_down_on() || response.clicked() || response.dragged() {
            response.request_focus();
            if let Some(pointer) = response.interact_pointer_pos() {
                let t = anatomy.t(pointer.x);
                let next = allowed.clamp(total.detent(t, detents));
                *value = N::from_f64(if N::INTEGRAL { next.round() } else { next });
            }
        }
        if response.has_focus() {
            let key = ui.input_mut(|input| {
                let home = input.consume_key(Modifiers::NONE, Key::Home);
                let end = input.consume_key(Modifiers::NONE, Key::End);
                let steps = input.count_and_consume_key(Modifiers::NONE, Key::ArrowRight) as i32
                    - input.count_and_consume_key(Modifiers::NONE, Key::ArrowLeft) as i32;
                (home, end, steps)
            });
            let next = match key {
                (true, _, _) => Some(allowed.lo),
                (_, true, _) => Some(allowed.hi),
                (_, _, 0) => None,
                (_, _, steps) => Some(allowed.clamp(
                    value.to_f64()
                        + f64::from(steps) * total.width()
                            / f64::from(detents.get().saturating_sub(1)),
                )),
            };
            if let Some(next) = next {
                *value = N::from_f64(if N::INTEGRAL { next.round() } else { next });
            }
        }
        *value = N::from_f64(allowed.clamp(value.to_f64()));
        if *value != before {
            response.mark_changed();
        }
        response.widget_info(|| egui::WidgetInfo::slider(ui.is_enabled(), value.to_f64(), ""));

        let pose = Pose {
            carriage: total.t(value.to_f64()),
            gate_lo: total.t(allowed.lo),
            gate_hi: total.t(allowed.hi),
        };
        paint(ui, anatomy, pose, detents, &response);
        let wakes = wakes(ui, response.id, anatomy, pose);
        RailResponse { response, wakes }
    }
}

/// Preserves the established integer convenience surface while returning the
/// rail's physical displacement alongside egui's response.
pub fn rail_u16(ui: &mut egui::Ui, value: &mut u16, range: RangeInclusive<u16>) -> RailResponse {
    Rail::new(value, range).show(ui)
}

/// Display a [`u16`] [`Rail`] at an explicit logical-point width.
pub fn rail_u16_sized(
    ui: &mut egui::Ui,
    value: &mut u16,
    range: RangeInclusive<u16>,
    width: f32,
) -> RailResponse {
    Rail::new(value, range).width(width).show(ui)
}

#[must_use = "the response carries both egui state and displaced-water wakes"]
/// Interaction state and displaced-water geometry from one [`Rail`] frame.
///
/// This dereferences to `egui::Response`, so methods such as
/// `egui::Response::changed` remain directly available.
pub struct RailResponse {
    response: egui::Response,
    wakes: [Option<RailWake>; 3],
}

impl RailResponse {
    /// Iterate over every solid swept since the preceding frame.
    ///
    /// A frame may contain wakes from the carriage and either dynamic stop.
    pub fn wakes(&self) -> impl Iterator<Item = RailWake> + '_ {
        self.wakes.iter().copied().flatten()
    }

    /// Discard the water wakes and return the ordinary egui response.
    pub fn into_response(self) -> egui::Response {
        self.response
    }
}

impl Deref for RailResponse {
    type Target = egui::Response;

    fn deref(&self) -> &Self::Target {
        &self.response
    }
}

/// The signed, projected volume swept by one moving solid in a rail.
#[derive(Clone, Copy, Debug)]
pub struct RailWake {
    pub(crate) rect: Rect,
    pub(crate) travel: f32,
    pub(crate) area: f32,
}

impl RailWake {
    /// The swept screen-space envelope in logical egui points.
    pub fn rect(self) -> Rect {
        self.rect
    }

    /// Signed horizontal travel in logical egui points; positive is rightward.
    pub fn travel(self) -> f32 {
        self.travel
    }

    /// Projected displaced area in logical point².
    pub fn swept_area(self) -> f32 {
        self.area
    }
}

#[derive(Clone, Copy)]
struct Span {
    lo: f64,
    hi: f64,
}

impl Span {
    fn refine<N: Numeric>(range: RangeInclusive<N>) -> Self {
        let lo = range.start().to_f64();
        let hi = range.end().to_f64();
        assert!(
            lo.is_finite() && hi.is_finite() && lo <= hi,
            "rail spans must be finite and ascending"
        );
        Self { lo, hi }
    }

    fn width(self) -> f64 {
        self.hi - self.lo
    }

    fn clamp(self, value: f64) -> f64 {
        value.clamp(self.lo, self.hi)
    }

    fn t(self, value: f64) -> f32 {
        if self.width() <= f64::EPSILON {
            0.5
        } else {
            ((value - self.lo) / self.width()).clamp(0.0, 1.0) as f32
        }
    }

    fn detent(self, t: f32, detents: NonZeroU16) -> f64 {
        let n = f64::from(detents.get().saturating_sub(1));
        let station = (f64::from(t) * n).round() / n;
        self.lo + station * self.width()
    }
}

#[derive(Clone, Copy)]
struct Anatomy {
    rect: Rect,
    slot: Rect,
    travel: [f32; 2],
}

impl Anatomy {
    fn new(rect: Rect) -> Self {
        let slot = Rect::from_center_size(rect.center(), Vec2::new(rect.width() - 4.0, SLOT_H));
        let travel = [
            slot.left() + HANDLE_DIAMETER * 0.5 + CLEARANCE,
            slot.right() - HANDLE_DIAMETER * 0.5 - CLEARANCE,
        ];
        Self { rect, slot, travel }
    }

    fn x(self, t: f32) -> f32 {
        egui::lerp(self.travel[0]..=self.travel[1], t)
    }

    fn t(self, x: f32) -> f32 {
        let [lo, hi] = self.travel;
        ((x - lo) / (hi - lo)).clamp(0.0, 1.0)
    }

    fn handle(self, t: f32) -> Rect {
        Rect::from_center_size(
            Pos2::new(self.x(t), self.rect.center().y),
            Vec2::new(HANDLE_DIAMETER, HANDLE_H),
        )
    }
}

#[derive(Clone, Copy, Debug)]
struct Pose {
    carriage: f32,
    gate_lo: f32,
    gate_hi: f32,
}

fn wakes(ui: &egui::Ui, id: egui::Id, anatomy: Anatomy, pose: Pose) -> [Option<RailWake>; 3] {
    let prior = ui.ctx().data_mut(|data| {
        let prior = data.get_temp::<Pose>(id.with("rail-pose"));
        let _old = data.insert_temp(id.with("rail-pose"), pose);
        prior
    });
    let Some(prior) = prior else {
        return [None; 3];
    };
    [
        sweep(
            anatomy,
            prior.carriage,
            pose.carriage,
            HANDLE_DIAMETER,
            HANDLE_H,
        ),
        sweep(anatomy, prior.gate_lo, pose.gate_lo, 0.0, SLOT_H),
        sweep(anatomy, prior.gate_hi, pose.gate_hi, 0.0, SLOT_H),
    ]
}

fn sweep(anatomy: Anatomy, from: f32, to: f32, width: f32, blade: f32) -> Option<RailWake> {
    let travel = anatomy.x(to) - anatomy.x(from);
    if travel.abs() < 0.05 {
        return None;
    }
    let a = Rect::from_center_size(
        Pos2::new(anatomy.x(from), anatomy.slot.center().y),
        Vec2::new(width.max(1.0), blade),
    );
    let b = Rect::from_center_size(
        Pos2::new(anatomy.x(to), anatomy.slot.center().y),
        Vec2::new(width.max(1.0), blade),
    );
    Some(RailWake {
        rect: a.union(b),
        travel,
        area: travel.abs() * blade,
    })
}

fn paint(
    ui: &egui::Ui,
    anatomy: Anatomy,
    pose: Pose,
    detents: NonZeroU16,
    response: &egui::Response,
) {
    let painter = ui.painter();
    foundry::socket_bed(painter, anatomy.slot);

    paint_linkage(painter, anatomy, pose.carriage);
    paint_gate(painter, anatomy, pose.gate_lo, true);
    paint_gate(painter, anatomy, pose.gate_hi, false);
    foundry::socket_rim(painter, anatomy.slot);
    paint_notches(painter, anatomy, detents);
    paint_handle(painter, anatomy.handle(pose.carriage));
    if response.has_focus() {
        let _focus = painter.rect_stroke(
            anatomy.rect.shrink(1.0),
            1.0,
            Stroke::new(1.0_f32, HOT.gamma_multiply(0.42)),
            egui::StrokeKind::Inside,
        );
    }
}

/// Offset slider-crank inverse kinematics. The carriage is the prismatic pair;
/// the two circle constraints solve the crank pin exactly for every position.
/// Only the connecting rod above that pin is visible through the aperture.
const HALF_STROKE: f32 = 0.82;
const JOURNAL_DEPTH: f32 = 1.35;
const CRANK: f32 = 0.88;
const CONROD: f32 = 1.22;
const RECESS_PERSPECTIVE: f32 = 0.16;
const ROD_RADIUS: f32 = 1.25;

/// Coordinates in the linkage plane. `z` is positive into the recess, hence
/// opposite the common universe's viewer-facing +z axis.
#[derive(Clone, Copy, Debug)]
struct Xz {
    x: f32,
    z: f32,
}

impl Xz {
    const fn new(x: f32, z: f32) -> Self {
        Self { x, z }
    }

    fn distance(self, rhs: Self) -> f32 {
        (self.x - rhs.x).hypot(self.z - rhs.z)
    }
}

fn carriage_xz(t: f32) -> Xz {
    Xz::new(egui::lerp(-HALF_STROKE..=HALF_STROKE, t), 0.0)
}

fn crank_pin(t: f32) -> Xz {
    let carriage = carriage_xz(t);
    let journal = Xz::new(0.0, JOURNAL_DEPTH);
    let ray = Xz::new(carriage.x - journal.x, carriage.z - journal.z);
    let d = carriage.distance(journal);
    let q = (CRANK * CRANK - CONROD * CONROD + d * d) / (2.0 * d);
    let wing = (CRANK * CRANK - q * q).max(0.0).sqrt();
    let u = Xz::new(ray.x / d, ray.z / d);
    Xz::new(
        journal.x + q * u.x - wing * u.z,
        journal.z + q * u.z + wing * u.x,
    )
}

fn depth_scale(z: f32) -> f32 {
    (1.0 + RECESS_PERSPECTIVE * z / JOURNAL_DEPTH).recip()
}

fn project_xz(anatomy: Anatomy, point: Xz) -> Pos2 {
    let half = (anatomy.travel[1] - anatomy.travel[0]) * 0.5;
    Pos2::new(
        anatomy.slot.center().x + point.x / HALF_STROKE * half * depth_scale(point.z),
        anatomy.slot.center().y,
    )
}

fn tapered_quad(segment: [Pos2; 2], radii: [f32; 2]) -> Vec<Pos2> {
    let tangent = (segment[1] - segment[0]).normalized();
    let normal = Vec2::new(-tangent.y, tangent.x);
    vec![
        segment[0] + normal * radii[0],
        segment[1] + normal * radii[1],
        segment[1] - normal * radii[1],
        segment[0] - normal * radii[0],
    ]
}

fn recess_irradiance(z: f32) -> f32 {
    0.66 * (1.0 - 0.12 * (z / JOURNAL_DEPTH).clamp(0.0, 1.0))
}

fn rod_mesh(
    segment: [Pos2; 2],
    radii: [f32; 2],
    front_normal_z: f32,
    irradiance: [f32; 2],
) -> egui::Mesh {
    const BANDS: u32 = 8;
    let tangent = (segment[1] - segment[0]).normalized();
    let normal = Vec2::new(-tangent.y, tangent.x);
    let mut mesh = egui::Mesh::default();
    for band in 0..=BANDS {
        let section = band as f32 / BANDS as f32 * 2.0 - 1.0;
        let nz = (1.0 - section * section).max(0.0).sqrt() * front_normal_z;
        let bronze = foundry::turned_bronze(section, nz);
        mesh.colored_vertex(
            segment[0] + normal * section * radii[0],
            bronze.gamma_multiply(irradiance[0]),
        );
        mesh.colored_vertex(
            segment[1] + normal * section * radii[1],
            bronze.gamma_multiply(irradiance[1]),
        );
        if band > 0 {
            let base = (band - 1) * 2;
            mesh.add_triangle(base, base + 1, base + 2);
            mesh.add_triangle(base + 1, base + 3, base + 2);
        }
    }
    mesh
}

/// Edge-on projection of a pinned clevis joint. The rod eye and both clevis
/// ears are circular plates in x-z, so the viewer sees only their bevelled
/// y-thickness; the clevis pin itself crosses the stack on the y axis.
fn paint_clevis(painter: &egui::Painter, pin: Pos2, scale: f32, dim: f32) {
    let ear_offset = 2.20 * scale;
    let ear_radius = Vec2::new(4.25, 0.58) * scale;
    let eye_radius = Vec2::new(3.60, 0.64) * scale;
    let upper = pin - Vec2::Y * ear_offset;
    let lower = pin + Vec2::Y * ear_offset;

    let _lower_ear = painter.add(egui::Shape::ellipse_filled(
        lower,
        ear_radius,
        bronze(0.34).gamma_multiply(dim),
    ));
    let _upper_ear = painter.add(egui::Shape::ellipse_filled(
        upper,
        ear_radius,
        bronze(0.58).gamma_multiply(dim),
    ));
    let _eye = painter.add(egui::Shape::ellipse_filled(
        pin,
        eye_radius,
        bronze(0.46).gamma_multiply(dim),
    ));
    let _eye_bore = painter.add(egui::Shape::ellipse_filled(
        pin,
        Vec2::new(1.12, 0.30) * scale,
        Color32::from_rgb(11, 8, 6),
    ));

    let pin_span = ear_offset + ear_radius.y;
    let _pin_shadow = painter.line_segment(
        [pin - Vec2::Y * pin_span, pin + Vec2::Y * pin_span],
        Stroke::new(1.30_f32 * scale, bronze(0.12).gamma_multiply(dim)),
    );
    let _clevis_pin = painter.line_segment(
        [pin - Vec2::Y * pin_span, pin + Vec2::Y * pin_span],
        Stroke::new(0.58_f32 * scale, bronze(0.72).gamma_multiply(dim)),
    );
    let _pin_head = painter.add(egui::Shape::ellipse_filled(
        pin - Vec2::Y * pin_span,
        Vec2::new(0.72, 0.25) * scale,
        bronze(0.78).gamma_multiply(dim),
    ));
}

fn paint_linkage(painter: &egui::Painter, anatomy: Anatomy, t: f32) {
    let carriage_xz = carriage_xz(t);
    let pin_xz = crank_pin(t);
    let carriage = project_xz(anatomy, carriage_xz);
    let pin = project_xz(anatomy, pin_xz);
    let submerged = painter.with_clip_rect(anatomy.slot.shrink(1.0));
    let pin_scale = depth_scale(pin_xz.z);
    let radii = [ROD_RADIUS, ROD_RADIUS * pin_scale];
    let segment = [carriage, pin];
    let _shadow = submerged.add(egui::Shape::convex_polygon(
        tapered_quad(
            [carriage + Vec2::Y * 0.85, pin + Vec2::Y * 0.85],
            [radii[0] + 0.42, radii[1] + 0.36],
        ),
        Color32::from_black_alpha(155),
        Stroke::NONE,
    ));
    let _silhouette = submerged.add(egui::Shape::convex_polygon(
        tapered_quad(segment, [radii[0] + 0.24, radii[1] + 0.20]),
        bronze(0.08),
        Stroke::NONE,
    ));
    let front_normal_z = ((pin_xz.x - carriage_xz.x) / CONROD).abs();
    let _rod = submerged.add(egui::Shape::mesh(rod_mesh(
        segment,
        radii,
        front_normal_z,
        [
            recess_irradiance(carriage_xz.z),
            recess_irradiance(pin_xz.z),
        ],
    )));
    paint_clevis(&submerged, pin, pin_scale, recess_irradiance(pin_xz.z));
}

fn paint_gate(painter: &egui::Painter, anatomy: Anatomy, boundary: f32, left: bool) {
    let face = anatomy.x(boundary)
        + if left {
            -HANDLE_DIAMETER * 0.5 - CLEARANCE
        } else {
            HANDLE_DIAMETER * 0.5 + CLEARANCE
        };
    let aperture = anatomy.slot.shrink(foundry::RIM_WIDTH);
    let plate = if left {
        Rect::from_min_max(aperture.min, Pos2::new(face, aperture.bottom()))
    } else {
        Rect::from_min_max(Pos2::new(face, aperture.top()), aperture.max)
    };
    if plate.width() <= 1.0 {
        return;
    }
    let _body = painter.rect_filled(plate, 0.0, bronze(0.12));
    let _upper_seat = painter.line_segment(
        [plate.left_top(), plate.right_top()],
        Stroke::new(1.2_f32, Color32::from_black_alpha(170)),
    );
    let _lower_seat = painter.line_segment(
        [plate.left_bottom(), plate.right_bottom()],
        Stroke::new(0.8_f32, bronze(0.26)),
    );
    let hatch = painter.with_clip_rect(plate.shrink(0.5));
    let pitch = 6.0;
    let mut x = plate.left() - plate.height();
    while x <= plate.right() + plate.height() {
        let _slash = hatch.line_segment(
            [
                Pos2::new(x, plate.bottom()),
                Pos2::new(x + plate.height(), plate.top()),
            ],
            Stroke::new(0.75_f32, bronze(0.48).gamma_multiply(0.68)),
        );
        let _backslash = hatch.line_segment(
            [
                Pos2::new(x, plate.top()),
                Pos2::new(x + plate.height(), plate.bottom()),
            ],
            Stroke::new(0.55_f32, Color32::from_black_alpha(105)),
        );
        x += pitch;
    }
    let x = if left { plate.right() } else { plate.left() };
    let voidward = if left { 1.0 } else { -1.0 };
    let _stop_shadow = painter.line_segment(
        [
            Pos2::new(x + voidward, plate.top()),
            Pos2::new(x + voidward, plate.bottom()),
        ],
        Stroke::new(2.0_f32, Color32::from_black_alpha(145)),
    );
    let _stop = painter.line_segment(
        [Pos2::new(x, plate.top()), Pos2::new(x, plate.bottom())],
        Stroke::new(1.1_f32, bronze(0.72)),
    );
}

fn paint_notches(painter: &egui::Painter, anatomy: Anatomy, detents: NonZeroU16) {
    for station in 0..detents.get() {
        let t = f32::from(station) / f32::from(detents.get() - 1);
        let x = anatomy.x(t);
        let crown = [
            Pos2::new(x - 3.0, anatomy.slot.top() - 5.0),
            Pos2::new(x + 3.0, anatomy.slot.top() - 5.0),
        ];
        let tip = Pos2::new(x, anatomy.slot.top() + 0.7);
        foundry::stamp(
            painter,
            vec![crown[0], crown[1], tip],
            &[crown],
            &[[crown[1], tip], [tip, crown[0]]],
            0.0,
        );
    }
}

fn paint_handle(painter: &egui::Painter, rect: Rect) {
    let shadow = rect.translate(Vec2::new(0.0, 1.8)).expand(0.8);
    let _shadow = painter.rect_filled(shadow, 0.0, Color32::from_black_alpha(170));
    foundry::cylinder(painter, rect, StockAxis::ScreenY);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linkage_honors_both_rigid_members_across_the_stroke() {
        let journal = Xz::new(0.0, JOURNAL_DEPTH);
        for i in 0..=40 {
            let t = i as f32 / 40.0;
            let carriage = carriage_xz(t);
            let pin = crank_pin(t);
            assert!((pin.distance(journal) - CRANK).abs() < 1e-5);
            assert!((pin.distance(carriage) - CONROD).abs() < 1e-5);
        }
    }

    #[test]
    fn recess_projection_tapers_the_rod_without_bending_its_axis() {
        let anatomy = Anatomy::new(Rect::from_min_size(Pos2::ZERO, Vec2::new(320.0, H)));
        for i in 0..=40 {
            let t = i as f32 / 40.0;
            let carriage = carriage_xz(t);
            let pin = crank_pin(t);
            assert!(pin.z > 0.0);
            assert!(depth_scale(pin.z) < depth_scale(0.0));
            assert_eq!(project_xz(anatomy, carriage).y, project_xz(anatomy, pin).y);
        }
    }

    #[test]
    fn detents_belong_to_the_total_span_not_the_admissible_span() {
        let total = Span { lo: 0.0, hi: 10.0 };
        let eleven = NonZeroU16::new(11).unwrap_or(NonZeroU16::MIN);
        assert_eq!(total.detent(0.73, eleven), 7.0);
        assert_eq!(
            Span { lo: 0.0, hi: 5.0 }.clamp(total.detent(0.73, eleven)),
            5.0
        );
    }

    #[test]
    fn pointer_click_enters_the_requested_detent_and_sweeps_water() {
        let ctx = egui::Context::default();
        let mut value = 4_u16;
        let mut swept = 0.0;
        let _prime = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(320.0, H))),
                ..egui::RawInput::default()
            },
            |ui| {
                let _rail = Rail::new(&mut value, 0..=10)
                    .detents(11)
                    .width(320.0)
                    .show(ui);
            },
        );
        for pressed in [true, false] {
            let _frame = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(320.0, H))),
                    events: vec![
                        egui::Event::PointerMoved(Pos2::new(270.0, H * 0.5)),
                        egui::Event::PointerButton {
                            pos: Pos2::new(270.0, H * 0.5),
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: Modifiers::NONE,
                        },
                    ],
                    ..egui::RawInput::default()
                },
                |ui| {
                    let rail = Rail::new(&mut value, 0..=10)
                        .detents(11)
                        .width(320.0)
                        .show(ui);
                    swept += rail.wakes().map(RailWake::swept_area).sum::<f32>();
                },
            );
        }
        assert_eq!(value, 9);
        assert!(swept > 3_000.0, "carriage swept only {swept} px²");
    }
}

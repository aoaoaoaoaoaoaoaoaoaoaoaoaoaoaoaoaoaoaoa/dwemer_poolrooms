//! The common Poolrooms foundry: one illuminant, one bronze charge, one set of
//! dies. Screen coordinates lie in the x-y plane with +y downward; the viewer
//! lies on +z. The distant key is confined to the y-z plane at 60° above the
//! top-of-screen horizon, so L=(0, -½, √3/2). Every authored metal part is cut
//! from these responses rather than carrying a private imitation.

use egui::{Color32, Pos2, Rect, Stroke};

pub(crate) const ABYSS: Color32 = Color32::from_rgb(3, 3, 4);
pub(crate) const CONTROL_STOCK_DIAMETER: f32 = 14.0;
pub(crate) const RIM_RADIUS: f32 = 1.0;
pub(crate) const RIM_WIDTH: f32 = 1.0;

const BRONZE_SHADOW: [f32; 3] = [34.0, 28.0, 19.0];
const BRONZE_BODY: [f32; 3] = [104.0, 86.0, 58.0];
const BRONZE_GLINT: [f32; 3] = [196.0, 170.0, 124.0];

const LIGHT_Y: f32 = -0.5;
const LIGHT_Z: f32 = 0.866_025_4;
const HALF_Y: f32 = -0.258_819_04;
const HALF_Z: f32 = 0.965_925_8;
const METAL_SHINE: f32 = 14.0;
const STAMP_GAUGE: f32 = 1.0;

#[derive(Clone, Copy)]
pub(crate) enum StockAxis {
    ScreenX,
    ScreenY,
}

/// Diffuse and Blinn-Phong response from the y and z components of a unit
/// normal. The omitted x component is immaterial because the illuminant and
/// half-vector both lie in the y-z plane.
pub(crate) fn yz_lumen(ny: f32, nz: f32, shine: f32) -> (f32, f32) {
    let diffuse = (ny * LIGHT_Y + nz * LIGHT_Z).max(0.0);
    let specular = (ny * HALF_Y + nz * HALF_Z).max(0.0).powf(shine);
    (diffuse, specular)
}

/// The foundry's oxidized-bronze ramp. `tone` is illumination, not a new
/// material choice: shadow, body, and polished glint are fixed alloy swatches.
pub(crate) fn bronze(tone: f32) -> Color32 {
    let tone = tone.clamp(0.0, 1.0);
    let (lo, hi, t) = if tone < 0.6 {
        (BRONZE_SHADOW, BRONZE_BODY, tone / 0.6)
    } else {
        (BRONZE_BODY, BRONZE_GLINT, (tone - 0.6) / 0.4)
    };
    let channel = |i: usize| (lo[i] + (hi[i] - lo[i]) * t).round() as u8;
    Color32::from_rgb(channel(0), channel(1), channel(2))
}

/// Bronze cut on a lathe, evaluated under the foundry illuminant. `ny` and
/// `nz` are the visible surface normal's components in the common universe.
pub(crate) fn turned_bronze(ny: f32, nz: f32) -> Color32 {
    let (diffuse, specular) = yz_lumen(ny, nz, METAL_SHINE);
    bronze(0.16 + 0.5 * diffuse + 0.8 * specular)
}

/// Orthographic cylindrical stock with a strictly untapered silhouette. A
/// screen-x roller and a screen-y handle share the same circular section; only
/// the section's orientation against the global light differs.
pub(crate) fn cylinder(painter: &egui::Painter, rect: Rect, axis: StockAxis) {
    const BANDS: usize = 14;
    let mut mesh = egui::Mesh::default();
    for band in 0..=BANDS {
        let f = band as f32 / BANDS as f32;
        let s = f * 2.0 - 1.0;
        let nz = (1.0 - s * s).max(0.0).sqrt();
        let ny = match axis {
            StockAxis::ScreenX => s,
            StockAxis::ScreenY => 0.0,
        };
        let color = turned_bronze(ny, nz);
        match axis {
            StockAxis::ScreenX => {
                let y = egui::lerp(rect.top()..=rect.bottom(), f);
                mesh.colored_vertex(Pos2::new(rect.left(), y), color);
                mesh.colored_vertex(Pos2::new(rect.right(), y), color);
            }
            StockAxis::ScreenY => {
                let x = egui::lerp(rect.left()..=rect.right(), f);
                mesh.colored_vertex(Pos2::new(x, rect.top()), color);
                mesh.colored_vertex(Pos2::new(x, rect.bottom()), color);
            }
        }
        if band > 0 {
            let base = (band as u32 - 1) * 2;
            mesh.add_triangle(base, base + 1, base + 2);
            mesh.add_triangle(base + 1, base + 3, base + 2);
        }
    }
    let _stock = painter.add(egui::Shape::mesh(mesh));
    let _silhouette = painter.rect_stroke(
        rect,
        0.0,
        Stroke::new(0.7_f32, bronze(0.12)),
        egui::StrokeKind::Inside,
    );
}

/// Paint the abyss and its machined inner walls. Contents are inserted after
/// this pass; [`socket_rim`] is struck last so every assembly seats under it.
pub(crate) fn socket_bed(painter: &egui::Painter, rect: Rect) {
    let _void = painter.rect_filled(rect, RIM_RADIUS, ABYSS);
    let _shadow = painter.line_segment(
        [rect.left_top(), rect.right_top()],
        Stroke::new(1.6_f32, Color32::from_rgb(1, 1, 2)),
    );
    let _catch = painter.line_segment(
        [rect.left_bottom(), rect.right_bottom()],
        Stroke::new(RIM_WIDTH, bronze(0.26)),
    );
}

pub(crate) fn socket_rim(painter: &egui::Painter, rect: Rect) {
    let _rim = painter.rect_stroke(
        rect,
        RIM_RADIUS,
        Stroke::new(RIM_WIDTH, bronze(0.13)),
        egui::StrokeKind::Inside,
    );
}

/// A flat part stamped from the common bronze sheet. `crowns` face the global
/// key; `soles` face away. Supplying those die edges gives arrows and detents
/// exactly the same body, crown, and undercut.
pub(crate) fn stamp(
    painter: &egui::Painter,
    silhouette: Vec<Pos2>,
    crowns: &[[Pos2; 2]],
    soles: &[[Pos2; 2]],
    dim: f32,
) {
    let _body = painter.add(egui::Shape::convex_polygon(
        silhouette,
        bronze(0.60 + dim),
        Stroke::NONE,
    ));
    for edge in crowns {
        let _crown = painter.line_segment(*edge, Stroke::new(STAMP_GAUGE, bronze(0.86 + dim)));
    }
    for edge in soles {
        let _sole = painter.line_segment(*edge, Stroke::new(STAMP_GAUGE, bronze(0.18 + dim)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn illuminant_is_unit_length_and_sixty_degrees_above_the_horizon() {
        assert!((LIGHT_Y * LIGHT_Y + LIGHT_Z * LIGHT_Z - 1.0).abs() < 1e-6);
        assert!((LIGHT_Z.asin().to_degrees() - 60.0).abs() < 1e-4);
        assert!((HALF_Y * HALF_Y + HALF_Z * HALF_Z - 1.0).abs() < 1e-6);
    }
}

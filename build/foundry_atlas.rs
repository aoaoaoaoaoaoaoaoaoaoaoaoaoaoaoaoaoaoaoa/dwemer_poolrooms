//! Build-time forge for every fixed-camera foundry asset.

use std::{
    collections::HashMap,
    f32::consts::{FRAC_PI_2, PI, TAU},
    fs::File,
    io::{self, BufWriter, Write},
    path::Path,
};

use brass_foundry::{
    Charge, Mesh as Compiled, Model, Pixel, RustDialect, RustReach, Vec3 as V3, Vertex,
    compile_with as compile_bronze_with, darkened_lit_with_key, depth, emit_rust_as,
    forge as forge_bronze, lit_with_key, project, shadow as compile_shadow,
    shadow_source as compile_shadow_source, visible,
};

use crate::foundry_law::{
    DARK_AMBIENT, DARK_BROAD_SHINE, DARK_BROAD_WEIGHT, DARK_DIFFUSE_WEIGHT, DARK_EXPOSURE,
    DARK_GLINT_SHINE, DARK_GLINT_WEIGHT, DARK_REFLECTION_CELL, DARK_TONE_CEILING, EYE_Z, HALF_Y,
    HALF_Z, LIGHT_Y, LIGHT_Z, MECHANISM_SIDE_LARGE, MECHANISM_SIDE_MEDIUM, MECHANISM_SIDE_SMALL,
    MECHANISM_SIDES, MOMENTARY_CASING_INSET, MONOGLYPH_LATCH, MONOGLYPH_PRESS, MONOGLYPH_REST,
    MomentaryGauge, RIM_WIDTH, bronze_rgb, material_terms, momentary_gauge, monoglyph_shade,
    polished_metal_tone,
};

const POSE_COUNT: usize = 32;
const TOP_HALF: f32 = 10.7;
const DISH_HALF: f32 = 8.1;
const BODY_HALF: f32 = 11.4;
const BEVEL_DEPTH: f32 = 0.8;
const BOWL_DEPTH: f32 = 1.35;
const LATCH_UP: f32 = 3.65;
const LATCH_STROKE: f32 = 31.05;
const PRESS_OVERTRAVEL: f32 = 6.30;
const LATCH_DOWN: f32 = LATCH_UP - LATCH_STROKE;
const POSE_MIN: f32 = LATCH_DOWN - PRESS_OVERTRAVEL;
const POSE_MAX: f32 = 4.22;
/// The shaft continues beyond the deepest rendered pose; its endpoint is
/// deliberately outside the aperture's visible volume.
const BODY_ROOT: f32 = POSE_MIN - BEVEL_DEPTH - 2.4;

const MONOGLYPH_POSE_COUNT: usize = 32;
const MONOGLYPH_BEVEL_DEPTH: f32 = 0.95;
const MONOGLYPH_POSE_MIN: f32 = -7.55;
const MONOGLYPH_POSE_MAX: f32 = 4.30;
const MONOGLYPH_BODY_ROOT: f32 = -11.0;
const SORT_POSE_COUNT: usize = 24;
const SORT_RETRACT: f32 = -5.8;
const SORT_REST: f32 = 2.3;
const SORT_CEILING: f32 = 4.4;
const SORT_POINTER_HALF_X: f32 = 5.4;
const SORT_POINTER_HALF_Y: f32 = 5.8;
const SORT_POINTER_DEPTH: f32 = 1.35;
const CLOSE_DENT_DEPTH: f32 = 2.95;
const CLOSE_FLOOR_HALF: f32 = 0.34;
const CLOSE_MOUTH_HALF: f32 = 1.08;
const CLOSE_FLOOR_REACH: f32 = 6.55;
const CLOSE_MOUTH_REACH: f32 = 7.30;
const _: () = {
    assert!(MONOGLYPH_POSE_MIN < MONOGLYPH_PRESS);
    assert!(MONOGLYPH_PRESS < MONOGLYPH_LATCH);
    assert!(MONOGLYPH_LATCH < MONOGLYPH_REST);
    assert!(MONOGLYPH_REST < MONOGLYPH_POSE_MAX);
    assert!(MONOGLYPH_BODY_ROOT < MONOGLYPH_POSE_MIN - MONOGLYPH_BEVEL_DEPTH);
};

const BAIL_POSE_COUNT: usize = 24;
const BAIL_REST: f32 = 0.17;
const BAIL_LIFT: f32 = 1.02;
const BAIL_POSE_MIN: f32 = 0.08;
const BAIL_POSE_MAX: f32 = 1.13;
const BAIL_HATCH_PITCH: f32 = 5.4;
const BAIL_HATCH_WIDTH: f32 = 0.42;
const BAIL_HATCH_RISE: f32 = 0.012;
const FRICTION_HATCH_PITCH: f32 = 4.4;
const FRICTION_HATCH_WIDTH: f32 = 1.0;
const FRICTION_HATCH_RISE: f32 = 0.22;
const _: () = {
    assert!(BAIL_POSE_MIN < BAIL_REST);
    assert!(BAIL_REST < BAIL_LIFT);
    assert!(BAIL_LIFT < BAIL_POSE_MAX);
    assert!(FRICTION_HATCH_PITCH >= 4.0);
    assert!(FRICTION_HATCH_WIDTH >= 1.0);
};

const GUARD_HALF: f32 = 16.8;
const GUARD_BASE: f32 = 0.72;
const GUARD_RISE: f32 = 6.30;
/// Fixed-stock guards need this additional crown rise as their footprint
/// contracts: unlike the plunger, their wire and frame radii do not scale.
const GUARD_STOCK_CLEARANCE_LIFT: f32 = 0.75;
const WIRE_RADIUS: f32 = 0.645;
const WIRE_LAYER: f32 = 0.66;
const FRAME_RADIUS: f32 = 0.72;
const WELD_RADIUS: f32 = 0.72;
const SMALL_WIRE_STATIONS: [f32; 2] = [-3.5, 3.5];
const MEDIUM_WIRE_STATIONS: [f32; 3] = [-7.0, 0.0, 7.0];
const LARGE_WIRE_STATIONS: [f32; 4] = [-10.5, -3.5, 3.5, 10.5];
const CURVE_STEPS: usize = 16;
const TUBE_SIDES: usize = 8;

// --- Reversible numerical thumbwheel ---------------------------------------
// One oblate foundry blank is cut in canonical XY, with its axle on Z. The two
// public planes are rigid transforms of this same solid; only after that
// transform do projection, key visibility, and illumination occur.
const WHEEL_STATIONS: usize = 12;
const WHEEL_POSE_COUNT: usize = 9;
const WHEEL_RADIAL_RINGS: usize = 20;
const WHEEL_LONGITUDES: usize = 96;
const WHEEL_RADIUS: f32 = 14.0;
const WHEEL_HALF_DEPTH: f32 = 5.2;
const WHEEL_SCALLOP_RADIUS: f32 = 8.75;
const WHEEL_SCALLOP_VERTEX: f32 = WHEEL_HALF_DEPTH / 3.0;
const WHEEL_SCALLOP_TANGENT_CURVATURE: f32 = 0.19;
const WHEEL_SCALLOP_RADIAL_CURVATURE: f32 = 0.34;
const WHEEL_SOCKET_SIDE: f32 = 24.0;
const WHEEL_APERTURE_TRAVEL: f32 = 14.0;
const WHEEL_APERTURE_AXIAL: f32 = 12.0;
const WHEEL_PITCH: f32 = TAU / WHEEL_STATIONS as f32;
const _: () = {
    assert!(WHEEL_STATIONS >= 8);
    assert!(WHEEL_POSE_COUNT >= 5);
    assert!(WHEEL_SCALLOP_VERTEX > 0.0);
    assert!(WHEEL_SCALLOP_VERTEX < WHEEL_HALF_DEPTH);
    assert!(WHEEL_APERTURE_TRAVEL < WHEEL_RADIUS * 2.0);
    assert!(WHEEL_APERTURE_AXIAL > WHEEL_HALF_DEPTH * 2.0);
    assert!(WHEEL_APERTURE_AXIAL <= WHEEL_SOCKET_SIDE - 4.0);
};

// --- Lead-screw scrollbar --------------------------------------------------
// The vertical screw uses a single-start right-handed thread. Unrolling the
// pitch cylinder gives lead/circumference = tan(30°), so translation and
// rotation share one exact kinematic law at runtime.
const SCROLL_SCREW_POSE_COUNT: usize = 33;
const SCROLL_CAP_POSE_COUNT: usize = 13;
const SCROLL_HELIX_STEPS: usize = 18;
const SCROLL_CAP_STATIONS: usize = 6;
const SCROLL_CAP_RADIAL_RINGS: usize = 10;
const SCROLL_CAP_LONGITUDES: usize = 48;
const SCROLL_SCREW_RADIUS: f32 = 1.55;
const SCROLL_THREAD_RADIUS: f32 = 0.48;
const SCROLL_PITCH_RADIUS: f32 = SCROLL_SCREW_RADIUS + 0.35 * SCROLL_THREAD_RADIUS;
const SCROLL_HELIX_TANGENT: f32 = 0.577_350_26;
const SCROLL_LEAD: f32 = TAU * SCROLL_PITCH_RADIUS * SCROLL_HELIX_TANGENT;
const SCROLL_CAP_RADIUS: f32 = 5.8;
const SCROLL_CAP_HALF_DEPTH: f32 = 4.0;
const SCROLL_COVE_RADIUS: f32 = 2.7;
const SCROLL_COVE_STATION: f32 = 5.65;
const SCROLL_COVE_CENTER_Z: f32 = 1.6;
const _: () = {
    assert!(SCROLL_SCREW_POSE_COUNT >= 17);
    assert!(SCROLL_CAP_POSE_COUNT >= 7);
    assert!(SCROLL_CAP_STATIONS == 6);
    assert!(SCROLL_COVE_CENTER_Z + SCROLL_COVE_RADIUS > SCROLL_CAP_HALF_DEPTH);
    assert!(SCROLL_COVE_CENTER_Z - SCROLL_COVE_RADIUS > -0.35 * SCROLL_CAP_HALF_DEPTH);
    assert!(SCROLL_COVE_CENTER_Z - SCROLL_COVE_RADIUS < 0.0);
};

// --- Dark-bronze material study -------------------------------------------
// Moving down the table transfers charge from the broad oxide bloom into a
// progressively tighter conductor glint. The last two rows deliberately raise
// specular gain as the lobe narrows: this is a contrast study, not an
// energy-neutral roughness proof, and its edges must remain visually decisive
// under the foundry's fixed incidence. Columns alter only exposure. The
// production coordinate is proven against the shared law below.
const MATERIAL_STUDY_PRODUCTION_ROW: usize = 4;
const MATERIAL_STUDY_PRODUCTION_COLUMN: usize = 3;
const MATERIAL_STUDY_EXPOSURES: [f32; 5] = [0.72, 0.94, 1.0, DARK_EXPOSURE, 1.4];

#[derive(Clone, Copy)]
struct StudyReflection {
    name: &'static str,
    broad_weight: f32,
    broad_shine: f32,
    glint_weight: f32,
    glint_shine: f32,
}

const MATERIAL_STUDY_ROWS: [StudyReflection; 5] = [
    StudyReflection {
        name: "MATTE",
        broad_weight: 0.28,
        broad_shine: 2.5,
        glint_weight: 0.06,
        glint_shine: 6.0,
    },
    StudyReflection {
        name: "SATIN",
        broad_weight: 0.15,
        broad_shine: 4.0,
        glint_weight: 0.28,
        glint_shine: 20.0,
    },
    StudyReflection {
        name: "LEGACY",
        broad_weight: 0.08,
        broad_shine: 6.0,
        glint_weight: 0.48,
        glint_shine: 48.0,
    },
    StudyReflection {
        name: "SPECULAR",
        broad_weight: 0.035,
        broad_shine: 8.0,
        glint_weight: 0.9,
        glint_shine: 80.0,
    },
    StudyReflection {
        name: "PRODUCTION",
        broad_weight: DARK_BROAD_WEIGHT,
        broad_shine: DARK_BROAD_SHINE,
        glint_weight: DARK_GLINT_WEIGHT,
        glint_shine: DARK_GLINT_SHINE,
    },
];
const _: () = {
    assert!(MATERIAL_STUDY_EXPOSURES[MATERIAL_STUDY_PRODUCTION_COLUMN] == DARK_EXPOSURE);
    assert!(MATERIAL_STUDY_ROWS[MATERIAL_STUDY_PRODUCTION_ROW].broad_weight == DARK_BROAD_WEIGHT);
    assert!(MATERIAL_STUDY_ROWS[MATERIAL_STUDY_PRODUCTION_ROW].broad_shine == DARK_BROAD_SHINE);
    assert!(MATERIAL_STUDY_ROWS[MATERIAL_STUDY_PRODUCTION_ROW].glint_weight == DARK_GLINT_WEIGHT);
    assert!(MATERIAL_STUDY_ROWS[MATERIAL_STUDY_PRODUCTION_ROW].glint_shine == DARK_GLINT_SHINE);
};

#[derive(Clone, Copy)]
struct BailGauge {
    side: u8,
    base_half: f32,
    face_half: f32,
    plate_rise: f32,
    hinge_y: f32,
    hinge_z: f32,
    span: f32,
    reach: f32,
    stock_radius: f32,
    rivet_offset: f32,
    rivet_radius: f32,
}

#[derive(Clone, Copy)]
struct FrictionGauge {
    side: u8,
    width: f32,
    base_half_x: f32,
    base_half_y: f32,
    face_half_x: f32,
    face_half_y: f32,
    plate_rise: f32,
    rivet_x: f32,
    rivet_y: f32,
    rivet_radius: f32,
}

#[derive(Clone, Copy)]
struct CheckboxGauge {
    side: u8,
    control_height: f32,
    assembly_side: f32,
    socket_half: f32,
    top_half: f32,
    dish_half: f32,
    body_half: f32,
    bevel_depth: f32,
    bowl_depth: f32,
    latch_up: f32,
    latch_down: f32,
    pose_min: f32,
    pose_max: f32,
    body_root: f32,
    guard: GuardGauge,
}

#[derive(Clone, Copy)]
struct GuardGauge {
    guard_half: f32,
    guard_base: f32,
    guard_rise: f32,
    wire_stations: &'static [f32],
}

#[derive(Clone, Copy)]
struct CloseGauge {
    plunger: MomentaryGauge,
    crown_cells: usize,
    floor_half: f32,
    mouth_half: f32,
    floor_reach: f32,
    mouth_reach: f32,
}

fn close_gauge(side: u8) -> CloseGauge {
    let scale = f32::from(side) / f32::from(MECHANISM_SIDE_LARGE);
    CloseGauge {
        plunger: momentary_gauge(side),
        crown_cells: usize::from(side) * 5 / 4,
        floor_half: CLOSE_FLOOR_HALF * scale,
        mouth_half: CLOSE_MOUTH_HALF * scale,
        floor_reach: CLOSE_FLOOR_REACH * scale,
        mouth_reach: CLOSE_MOUTH_REACH * scale,
    }
}

fn checkbox_gauge(side: u8) -> CheckboxGauge {
    let scale = f32::from(side) / f32::from(MECHANISM_SIDE_LARGE);
    CheckboxGauge {
        side,
        control_height: 42.0 * scale,
        assembly_side: 38.0 * scale,
        socket_half: 14.8 * scale,
        top_half: TOP_HALF * scale,
        dish_half: DISH_HALF * scale,
        body_half: BODY_HALF * scale,
        bevel_depth: BEVEL_DEPTH * scale,
        bowl_depth: BOWL_DEPTH * scale,
        latch_up: LATCH_UP * scale,
        latch_down: LATCH_DOWN * scale,
        pose_min: POSE_MIN * scale,
        pose_max: POSE_MAX * scale,
        body_root: BODY_ROOT * scale,
        guard: GuardGauge {
            guard_half: GUARD_HALF * scale,
            // Frame, mesh, and weld stock remain physically identical at every
            // gauge. Fewer wires, rather than hairline wires, make the compact
            // guards legible after rasterization.
            guard_base: GUARD_BASE,
            guard_rise: GUARD_RISE * scale + GUARD_STOCK_CLEARANCE_LIFT * (1.0 - scale),
            wire_stations: guard_wire_stations(side),
        },
    }
}

fn monoglyph_guard_gauge(side: u8) -> GuardGauge {
    let scale = f32::from(side) / f32::from(MECHANISM_SIDE_LARGE);
    GuardGauge {
        guard_half: f32::from(side) * 0.5 - FRAME_RADIUS - 0.4,
        guard_base: GUARD_BASE,
        // Monoglyph travel is gauge-invariant, so every guard clears the same
        // raised crown while its lattice contracts by removing wires. Fixed
        // wire stock consumes proportionally more clearance in compact cages.
        guard_rise: GUARD_RISE + 0.75 + 1.5 * (1.0 - scale),
        wire_stations: guard_wire_stations(side),
    }
}

const fn guard_wire_stations(side: u8) -> &'static [f32] {
    match side {
        ..=22 => &SMALL_WIRE_STATIONS,
        23..=28 => &MEDIUM_WIRE_STATIONS,
        29.. => &LARGE_WIRE_STATIONS,
    }
}

fn bail_gauge(side: u8) -> BailGauge {
    let s = f32::from(side);
    let base_half = s * 0.5 - 0.5;
    let plate_rise = 1.15;
    let stock_radius = s * 0.045;
    BailGauge {
        side,
        base_half,
        face_half: base_half - 1.15,
        plate_rise,
        hinge_y: -s * 0.15,
        hinge_z: plate_rise + stock_radius * 0.72,
        span: s * 0.25,
        reach: s * 0.29,
        stock_radius,
        rivet_offset: base_half - 1.65,
        rivet_radius: s * 0.031,
    }
}

fn friction_gauge(side: u8) -> FrictionGauge {
    let height = f32::from(side);
    let width = height * 0.5;
    let base_half_x = width * 0.5 - 0.5;
    let base_half_y = height * 0.5 - 0.5;
    let plate_rise = 1.15;
    let face_half_x = base_half_x - plate_rise;
    let face_half_y = base_half_y - plate_rise;
    let rivet_radius = height * 0.036;
    FrictionGauge {
        side,
        width,
        base_half_x,
        base_half_y,
        face_half_x,
        face_half_y,
        plate_rise,
        rivet_x: face_half_x - rivet_radius - 0.12,
        rivet_y: face_half_y - rivet_radius - 0.15,
        rivet_radius,
    }
}

#[derive(Clone, Copy)]
enum WheelPlane {
    XZ,
    YZ,
}

impl WheelPlane {
    const ALL: [Self; 2] = [Self::XZ, Self::YZ];

    const fn name(self) -> &'static str {
        match self {
            Self::XZ => "XZ",
            Self::YZ => "YZ",
        }
    }

    const fn aperture(self) -> [f32; 2] {
        match self {
            Self::XZ => [WHEEL_APERTURE_TRAVEL, WHEEL_APERTURE_AXIAL],
            Self::YZ => [WHEEL_APERTURE_AXIAL, WHEEL_APERTURE_TRAVEL],
        }
    }
}

fn wheel_pose(model: &Model, phase: f32, plane: WheelPlane) -> Model {
    model.transformed(
        |point| wheel_plane(point.rotate_z(phase), plane),
        |normal| wheel_plane(normal.rotate_z(phase), plane),
    )
}

const fn wheel_plane(point: V3, plane: WheelPlane) -> V3 {
    match plane {
        // Rᵧ(+π/2): the canonical Z axle becomes screen X.
        WheelPlane::YZ => V3::new(point.z, point.y, -point.x),
        // Rₓ(−π/2): the canonical Z axle becomes screen Y.
        WheelPlane::XZ => V3::new(point.x, point.z, -point.y),
    }
}

pub(crate) fn bake(
    checkbox_path: &Path,
    monoglyph_path: &Path,
    corner_close_path: &Path,
    drag_handle_path: &Path,
    number_input_path: &Path,
    screw_scroll_path: &Path,
    sort_toggle_path: &Path,
    material_study_path: &Path,
    longinus_cursor_path: &Path,
) -> io::Result<()> {
    verify_geometry();
    bake_checkbox(checkbox_path)?;
    bake_monoglyph(monoglyph_path)?;
    bake_corner_close(corner_close_path)?;
    bake_drag_handle(drag_handle_path)?;
    bake_number_input(number_input_path)?;
    bake_screw_scroll(screw_scroll_path)?;
    bake_sort_toggle(sort_toggle_path)?;
    bake_material_study(material_study_path)?;
    bake_longinus_cursor(longinus_cursor_path)
}

fn bake_checkbox(path: &Path) -> io::Result<()> {
    let mut out = BufWriter::new(File::create(path)?);
    writeln!(out, "// @generated by build/foundry_atlas.rs; do not edit.")?;
    writeln!(out, "pub(super) const POSE_COUNT: usize = {POSE_COUNT};")?;
    let gauge_count = MECHANISM_SIDES.len();
    writeln!(out, "pub(super) const GAUGE_COUNT: usize = {gauge_count};")?;
    writeln!(
        out,
        "pub(super) const SHADOW_EYE_Z: f32 = {};",
        scalar(EYE_Z)
    )?;
    writeln!(
        out,
        "pub(super) const SHADOW_SLOPE: f32 = {};",
        scalar(-LIGHT_Y / LIGHT_Z)
    )?;

    for side in MECHANISM_SIDES {
        let gauge = checkbox_gauge(side);
        let guard = guard(gauge.guard);
        let guard_mesh = compile_bronze(&guard, 0.96);
        let guard_floor_shadow = compile_shadow(&guard, 0.0, 46);
        let guard_crown_shadow = compile_shadow_source(&guard, gauge.pose_max, 18);
        emit_mesh(&mut out, &format!("GAUGE_{side}_GUARD"), &guard_mesh)?;
        emit_mesh(
            &mut out,
            &format!("GAUGE_{side}_GUARD_FLOOR_SHADOW"),
            &guard_floor_shadow,
        )?;
        emit_shadow(
            &mut out,
            &format!("GAUGE_{side}_GUARD_CROWN_SHADOW"),
            &guard_crown_shadow,
        )?;

        let poses = (0..POSE_COUNT)
            .map(|index| {
                let elevation = checkbox_pose_elevation(index, gauge);
                let depth = ((elevation - gauge.latch_down) / (gauge.latch_up - gauge.latch_down))
                    .clamp(0.0, 1.0);
                let button = plunger(elevation, gauge);
                (
                    elevation,
                    compile_darkened_bronze(&button, 0.76 + 0.24 * depth),
                    compile_shadow(&button, 0.0, 82),
                )
            })
            .collect::<Vec<_>>();
        for (index, (_, button, shadow)) in poses.iter().enumerate() {
            emit_mesh(&mut out, &format!("GAUGE_{side}_BUTTON_{index:02}"), button)?;
            emit_mesh(
                &mut out,
                &format!("GAUGE_{side}_BUTTON_SHADOW_{index:02}"),
                shadow,
            )?;
        }
        writeln!(
            out,
            "static GAUGE_{side}_POSES: [BakedPose; POSE_COUNT] = ["
        )?;
        for (index, (elevation, _, _)) in poses.iter().enumerate() {
            writeln!(
                out,
                "BakedPose {{ elevation: {}, button: GAUGE_{side}_BUTTON_{index:02}, shadow: GAUGE_{side}_BUTTON_SHADOW_{index:02} }},",
                scalar(*elevation)
            )?;
        }
        writeln!(out, "];")?;
    }

    writeln!(
        out,
        "pub(super) static GAUGES: [BakedCheckboxGauge; GAUGE_COUNT] = ["
    )?;
    for side in MECHANISM_SIDES {
        let gauge = checkbox_gauge(side);
        writeln!(
            out,
            "BakedCheckboxGauge {{ side: {}, control_height: {}, assembly_side: {}, socket_half: {}, body_half: {}, latch_up: {}, latch_down: {}, pose_min: {}, pose_max: {}, wire_count: {}, guard: BakedGuard {{ mesh: GAUGE_{side}_GUARD, floor_shadow: GAUGE_{side}_GUARD_FLOOR_SHADOW, crown_shadow: GAUGE_{side}_GUARD_CROWN_SHADOW_SOURCE }}, poses: &GAUGE_{side}_POSES }},",
            gauge.side,
            scalar(gauge.control_height),
            scalar(gauge.assembly_side),
            scalar(gauge.socket_half),
            scalar(gauge.body_half),
            scalar(gauge.latch_up),
            scalar(gauge.latch_down),
            scalar(gauge.pose_min),
            scalar(gauge.pose_max),
            gauge.guard.wire_stations.len(),
        )?;
    }
    writeln!(out, "];")
}

fn bake_corner_close(path: &Path) -> io::Result<()> {
    let mut out = BufWriter::new(File::create(path)?);
    writeln!(out, "// @generated by build/foundry_atlas.rs; do not edit.")?;
    for (name, value) in [
        ("POSE_MIN", MONOGLYPH_POSE_MIN),
        ("POSE_MAX", MONOGLYPH_POSE_MAX),
        ("REST", MONOGLYPH_REST),
        ("PRESS", MONOGLYPH_PRESS),
    ] {
        writeln!(out, "pub(super) const {name}: f32 = {};", scalar(value))?;
    }
    writeln!(
        out,
        "pub(super) const POSE_COUNT: usize = {MONOGLYPH_POSE_COUNT};"
    )?;
    let gauge_count = MECHANISM_SIDES.len();
    writeln!(out, "pub(super) const GAUGE_COUNT: usize = {gauge_count};")?;

    for side in MECHANISM_SIDES {
        let gauge = close_gauge(side);
        let socket = compile_darkened_bronze(&momentary_socket(gauge.plunger), 1.0);
        let poses = (0..MONOGLYPH_POSE_COUNT)
            .map(|index| {
                let elevation = monoglyph_pose_elevation(index);
                let button = corner_close_plunger(elevation, gauge);
                (
                    elevation,
                    compile_close_crown(&button, elevation, gauge),
                    compile_shadow(&button, 0.0, 82),
                )
            })
            .collect::<Vec<_>>();
        emit_mesh(&mut out, &format!("GAUGE_{side}_SOCKET"), &socket)?;
        for (index, (_, button, shadow)) in poses.iter().enumerate() {
            emit_mesh(&mut out, &format!("GAUGE_{side}_BUTTON_{index:02}"), button)?;
            emit_mesh(&mut out, &format!("GAUGE_{side}_SHADOW_{index:02}"), shadow)?;
        }
        writeln!(
            out,
            "static GAUGE_{side}_POSES: [BakedPose; POSE_COUNT] = ["
        )?;
        for (index, (elevation, _, _)) in poses.iter().enumerate() {
            writeln!(
                out,
                "BakedPose {{ elevation: {}, button: GAUGE_{side}_BUTTON_{index:02}, shadow: GAUGE_{side}_SHADOW_{index:02} }},",
                scalar(*elevation)
            )?;
        }
        writeln!(out, "];")?;
    }

    writeln!(
        out,
        "pub(super) static GAUGES: [BakedGauge; GAUGE_COUNT] = ["
    )?;
    for side in MECHANISM_SIDES {
        let gauge = close_gauge(side).plunger;
        writeln!(
            out,
            "BakedGauge {{ side: {side}, socket_half: {}, top_half: {}, body_half: {}, socket: GAUGE_{side}_SOCKET, poses: &GAUGE_{side}_POSES }},",
            scalar(gauge.socket_half),
            scalar(gauge.top_half),
            scalar(gauge.body_half),
        )?;
    }
    writeln!(out, "];")
}

fn bake_monoglyph(path: &Path) -> io::Result<()> {
    let mut out = BufWriter::new(File::create(path)?);
    writeln!(out, "// @generated by build/foundry_atlas.rs; do not edit.")?;
    for (name, value) in [
        ("POSE_MIN", MONOGLYPH_POSE_MIN),
        ("POSE_MAX", MONOGLYPH_POSE_MAX),
        ("REST", MONOGLYPH_REST),
        ("LATCH", MONOGLYPH_LATCH),
        ("PRESS", MONOGLYPH_PRESS),
    ] {
        writeln!(out, "pub(super) const {name}: f32 = {};", scalar(value))?;
    }
    writeln!(
        out,
        "pub(super) const POSE_COUNT: usize = {MONOGLYPH_POSE_COUNT};"
    )?;
    let gauge_count = MECHANISM_SIDES.len();
    writeln!(out, "pub(super) const GAUGE_COUNT: usize = {gauge_count};")?;
    writeln!(
        out,
        "pub(super) const SHADOW_EYE_Z: f32 = {};",
        scalar(EYE_Z)
    )?;
    writeln!(
        out,
        "pub(super) const SHADOW_SLOPE: f32 = {};",
        scalar(-LIGHT_Y / LIGHT_Z)
    )?;

    for side in MECHANISM_SIDES {
        let gauge = momentary_gauge(side);
        let guard = guard(monoglyph_guard_gauge(side));
        let guard_mesh = compile_bronze(&guard, 0.96);
        let guard_floor_shadow = compile_shadow(&guard, 0.0, 46);
        let guard_crown_shadow = compile_shadow_source(&guard, MONOGLYPH_POSE_MAX, 18);
        let socket = compile_darkened_bronze(&momentary_socket(gauge), 1.0);
        let poses = (0..MONOGLYPH_POSE_COUNT)
            .map(|index| {
                let elevation = monoglyph_pose_elevation(index);
                let button = monoglyph_plunger(elevation, gauge);
                (
                    elevation,
                    compile_darkened_crown(&button, elevation),
                    compile_shadow(&button, 0.0, 82),
                )
            })
            .collect::<Vec<_>>();
        emit_mesh(&mut out, &format!("GAUGE_{side}_GUARD"), &guard_mesh)?;
        emit_mesh(
            &mut out,
            &format!("GAUGE_{side}_GUARD_FLOOR_SHADOW"),
            &guard_floor_shadow,
        )?;
        emit_shadow(
            &mut out,
            &format!("GAUGE_{side}_GUARD_CROWN_SHADOW"),
            &guard_crown_shadow,
        )?;
        emit_mesh(&mut out, &format!("GAUGE_{side}_SOCKET"), &socket)?;
        for (index, (_, button, shadow)) in poses.iter().enumerate() {
            emit_mesh(&mut out, &format!("GAUGE_{side}_BUTTON_{index:02}"), button)?;
            emit_mesh(&mut out, &format!("GAUGE_{side}_SHADOW_{index:02}"), shadow)?;
        }
        writeln!(
            out,
            "static GAUGE_{side}_POSES: [BakedPose; POSE_COUNT] = ["
        )?;
        for (index, (elevation, _, _)) in poses.iter().enumerate() {
            writeln!(
                out,
                "BakedPose {{ elevation: {}, button: GAUGE_{side}_BUTTON_{index:02}, shadow: GAUGE_{side}_SHADOW_{index:02} }},",
                scalar(*elevation)
            )?;
        }
        writeln!(out, "];")?;
    }
    writeln!(
        out,
        "pub(super) static GAUGES: [BakedMonoglyphGauge; GAUGE_COUNT] = ["
    )?;
    for side in MECHANISM_SIDES {
        let gauge = momentary_gauge(side);
        writeln!(
            out,
            "BakedMonoglyphGauge {{ side: {side}, socket_half: {}, top_half: {}, body_half: {}, guard: BakedGuard {{ mesh: GAUGE_{side}_GUARD, floor_shadow: GAUGE_{side}_GUARD_FLOOR_SHADOW, crown_shadow: GAUGE_{side}_GUARD_CROWN_SHADOW_SOURCE }}, socket: GAUGE_{side}_SOCKET, poses: &GAUGE_{side}_POSES }},",
            scalar(gauge.socket_half),
            scalar(gauge.top_half),
            scalar(gauge.body_half),
        )?;
    }
    writeln!(out, "];")
}

fn bake_sort_toggle(path: &Path) -> io::Result<()> {
    let mut out = BufWriter::new(File::create(path)?);
    writeln!(out, "// @generated by build/foundry_atlas.rs; do not edit.")?;
    for (name, value) in [
        ("RETRACT", SORT_RETRACT),
        ("REST", SORT_REST),
        ("CEILING", SORT_CEILING),
    ] {
        writeln!(out, "pub(super) const {name}: f32 = {};", scalar(value))?;
    }
    writeln!(
        out,
        "pub(super) const POSE_COUNT: usize = {SORT_POSE_COUNT};"
    )?;
    let gauge_count = MECHANISM_SIDES.len();
    writeln!(out, "pub(super) const GAUGE_COUNT: usize = {gauge_count};")?;

    for side in MECHANISM_SIDES {
        let gauge = momentary_gauge(side);
        let poses = (0..SORT_POSE_COUNT)
            .map(|index| {
                let elevation = lerp(
                    SORT_RETRACT,
                    SORT_CEILING,
                    index as f32 / (SORT_POSE_COUNT - 1) as f32,
                );
                let ascending = sort_pointer(elevation, side, true);
                let descending = sort_pointer(elevation, side, false);
                (
                    elevation,
                    [
                        compile_darkened_bronze(&ascending, 1.0),
                        compile_darkened_bronze(&descending, 1.0),
                    ],
                    [
                        compile_shadow(&ascending, 0.0, 72),
                        compile_shadow(&descending, 0.0, 72),
                    ],
                )
            })
            .collect::<Vec<_>>();
        for (index, (_, pointers, shadows)) in poses.iter().enumerate() {
            for direction in 0..2 {
                emit_mesh(
                    &mut out,
                    &format!("GAUGE_{side}_POINTER_{index:02}_{direction}"),
                    &pointers[direction],
                )?;
                emit_mesh(
                    &mut out,
                    &format!("GAUGE_{side}_SHADOW_{index:02}_{direction}"),
                    &shadows[direction],
                )?;
            }
        }
        writeln!(
            out,
            "static GAUGE_{side}_POSES: [BakedSortPose; POSE_COUNT] = ["
        )?;
        for (index, _) in poses.iter().enumerate() {
            writeln!(
                out,
                "BakedSortPose {{ pointers: [GAUGE_{side}_POINTER_{index:02}_0, GAUGE_{side}_POINTER_{index:02}_1], shadows: [GAUGE_{side}_SHADOW_{index:02}_0, GAUGE_{side}_SHADOW_{index:02}_1] }},"
            )?;
        }
        writeln!(out, "];")?;
        debug_assert!(gauge.socket_half > SORT_POINTER_HALF_Y * f32::from(side) / 32.0);
    }
    writeln!(
        out,
        "pub(super) static GAUGES: [BakedSortGauge; GAUGE_COUNT] = ["
    )?;
    for side in MECHANISM_SIDES {
        let gauge = momentary_gauge(side);
        writeln!(
            out,
            "BakedSortGauge {{ side: {side}, socket_half: {}, pointer_area: {}, poses: &GAUGE_{side}_POSES }},",
            scalar(gauge.socket_half),
            scalar(
                SORT_POINTER_HALF_X
                    * SORT_POINTER_HALF_Y
                    * 1.72
                    * (f32::from(side) / f32::from(MECHANISM_SIDE_LARGE)).powi(2)
            )
        )?;
    }
    writeln!(out, "];")
}

fn bake_drag_handle(path: &Path) -> io::Result<()> {
    let mut out = BufWriter::new(File::create(path)?);
    writeln!(out, "// @generated by build/foundry_atlas.rs; do not edit.")?;
    for (name, value) in [
        ("POSE_MIN", BAIL_POSE_MIN),
        ("POSE_MAX", BAIL_POSE_MAX),
        ("REST", BAIL_REST),
        ("LIFT", BAIL_LIFT),
    ] {
        writeln!(out, "pub(super) const {name}: f32 = {};", scalar(value))?;
    }
    writeln!(
        out,
        "pub(super) const POSE_COUNT: usize = {BAIL_POSE_COUNT};"
    )?;
    let gauge_count = MECHANISM_SIDES.len();
    writeln!(out, "pub(super) const GAUGE_COUNT: usize = {gauge_count};")?;

    for side in MECHANISM_SIDES {
        let gauge = bail_gauge(side);
        let plate = bail_plate(gauge);
        let hardware = bail_hardware(gauge);
        let mut complete = plate.clone();
        complete.append(hardware.clone());
        let floor_shadow = compile_shadow(&complete, 0.0, 44);
        let static_shadow = compile_shadow(&hardware, gauge.plate_rise, 34);
        let plate = compile_darkened_bronze(&plate, 1.0);
        let hardware = compile_darkened_bronze(&hardware, 1.0);
        emit_mesh(
            &mut out,
            &format!("GAUGE_{side}_FLOOR_SHADOW"),
            &floor_shadow,
        )?;
        emit_mesh(&mut out, &format!("GAUGE_{side}_PLATE"), &plate)?;
        emit_mesh(
            &mut out,
            &format!("GAUGE_{side}_STATIC_SHADOW"),
            &static_shadow,
        )?;
        emit_mesh(&mut out, &format!("GAUGE_{side}_HARDWARE"), &hardware)?;

        let poses = (0..BAIL_POSE_COUNT)
            .map(|index| {
                let angle = bail_pose_angle(index);
                let bail = bail(gauge, angle);
                (
                    angle,
                    compile_shadow(&bail, gauge.plate_rise, 78),
                    compile_darkened_bronze(&bail, 1.0),
                )
            })
            .collect::<Vec<_>>();
        for (index, (_, shadow, bail)) in poses.iter().enumerate() {
            emit_mesh(
                &mut out,
                &format!("GAUGE_{side}_BAIL_SHADOW_{index:02}"),
                shadow,
            )?;
            emit_mesh(&mut out, &format!("GAUGE_{side}_BAIL_{index:02}"), bail)?;
        }
        writeln!(
            out,
            "static GAUGE_{side}_POSES: [BakedBailPose; POSE_COUNT] = ["
        )?;
        for (index, (angle, _, _)) in poses.iter().enumerate() {
            writeln!(
                out,
                "BakedBailPose {{ angle: {}, shadow: GAUGE_{side}_BAIL_SHADOW_{index:02}, bail: GAUGE_{side}_BAIL_{index:02} }},",
                scalar(*angle)
            )?;
        }
        writeln!(out, "];")?;
    }

    for side in MECHANISM_SIDES {
        let gauge = friction_gauge(side);
        let plate = friction_plate(gauge);
        let hardware = friction_hardware(gauge);
        let mut complete = plate.clone();
        complete.append(hardware.clone());
        let floor_shadow = compile_shadow(&complete, 0.0, 44);
        let static_shadow = compile_shadow(&hardware, gauge.plate_rise, 56);
        let plate = compile_darkened_bronze(&plate, 1.0);
        let hardware = compile_bronze(&hardware, 1.0);
        emit_mesh(
            &mut out,
            &format!("FRICTION_{side}_FLOOR_SHADOW"),
            &floor_shadow,
        )?;
        emit_mesh(&mut out, &format!("FRICTION_{side}_PLATE"), &plate)?;
        emit_mesh(
            &mut out,
            &format!("FRICTION_{side}_STATIC_SHADOW"),
            &static_shadow,
        )?;
        emit_mesh(&mut out, &format!("FRICTION_{side}_HARDWARE"), &hardware)?;
    }

    writeln!(
        out,
        "pub(super) static GAUGES: [BakedBailGauge; GAUGE_COUNT] = ["
    )?;
    for side in MECHANISM_SIDES {
        let gauge = bail_gauge(side);
        writeln!(
            out,
            "BakedBailGauge {{ side: {}, plate: GAUGE_{side}_PLATE, floor_shadow: GAUGE_{side}_FLOOR_SHADOW, static_shadow: GAUGE_{side}_STATIC_SHADOW, hardware: GAUGE_{side}_HARDWARE, sweep_per_radian: {}, poses: &GAUGE_{side}_POSES }},",
            gauge.side,
            scalar(bail_sweep_per_radian(gauge)),
        )?;
    }
    writeln!(out, "];")?;
    writeln!(
        out,
        "pub(super) static FRICTION_GAUGES: [BakedFrictionGauge; GAUGE_COUNT] = ["
    )?;
    for side in MECHANISM_SIDES {
        let gauge = friction_gauge(side);
        writeln!(
            out,
            "BakedFrictionGauge {{ side: {}, width: {}, plate: FRICTION_{side}_PLATE, floor_shadow: FRICTION_{side}_FLOOR_SHADOW, static_shadow: FRICTION_{side}_STATIC_SHADOW, hardware: FRICTION_{side}_HARDWARE }},",
            gauge.side,
            scalar(gauge.width),
        )?;
    }
    writeln!(out, "];")
}

fn bake_number_input(path: &Path) -> io::Result<()> {
    let blank = numerical_thumbwheel();
    let mut out = BufWriter::new(File::create(path)?);
    writeln!(out, "// @generated by build/foundry_atlas.rs; do not edit.")?;
    for (name, value) in [
        ("SOCKET_SIDE", WHEEL_SOCKET_SIDE),
        ("PITCH", WHEEL_PITCH),
        ("RADIUS", WHEEL_RADIUS),
        ("HALF_DEPTH", WHEEL_HALF_DEPTH),
    ] {
        writeln!(out, "pub(super) const {name}: f32 = {};", scalar(value))?;
    }
    writeln!(
        out,
        "pub(super) const POSE_COUNT: usize = {WHEEL_POSE_COUNT};"
    )?;

    for plane in WheelPlane::ALL {
        let name = plane.name();
        for index in 0..WHEEL_POSE_COUNT {
            let phase = wheel_pose_phase(index);
            let model = wheel_pose(&blank, phase, plane);
            let wheel = compile_wheel(&model, phase, plane);
            emit_mesh(&mut out, &format!("{name}_WHEEL_{index:02}"), &wheel)?;
        }
        writeln!(out, "static {name}_POSES: [BakedWheelPose; POSE_COUNT] = [")?;
        for index in 0..WHEEL_POSE_COUNT {
            writeln!(
                out,
                "BakedWheelPose {{ phase: {}, wheel: {name}_WHEEL_{index:02} }},",
                scalar(wheel_pose_phase(index))
            )?;
        }
        writeln!(out, "];")?;
    }

    writeln!(out, "pub(super) static PLANES: [BakedWheelPlane; 2] = [")?;
    for plane in WheelPlane::ALL {
        let name = plane.name();
        let [width, height] = plane.aperture();
        writeln!(
            out,
            "BakedWheelPlane {{ aperture: [{}, {}], poses: &{name}_POSES }},",
            scalar(width),
            scalar(height),
        )?;
    }
    writeln!(out, "];")
}

fn bake_screw_scroll(path: &Path) -> io::Result<()> {
    let mut out = BufWriter::new(File::create(path)?);
    writeln!(out, "// @generated by build/foundry_atlas.rs; do not edit.")?;
    writeln!(out, "pub(super) const LEAD: f32 = {};", scalar(SCROLL_LEAD))?;
    writeln!(
        out,
        "pub(super) const SCREW_POSE_COUNT: usize = {SCROLL_SCREW_POSE_COUNT};"
    )?;
    writeln!(
        out,
        "pub(super) const CAP_POSE_COUNT: usize = {SCROLL_CAP_POSE_COUNT};"
    )?;

    for index in 0..SCROLL_SCREW_POSE_COUNT {
        let phase = scroll_screw_phase(index);
        let screw = compile_bronze(&scroll_screw(phase), 1.02);
        emit_mesh(&mut out, &format!("SCREW_{index:02}"), &screw)?;
    }
    writeln!(
        out,
        "static SCREW_POSES: [BakedScrewPose; SCREW_POSE_COUNT] = ["
    )?;
    for index in 0..SCROLL_SCREW_POSE_COUNT {
        writeln!(
            out,
            "BakedScrewPose {{ phase: {}, mesh: SCREW_{index:02} }},",
            scalar(scroll_screw_phase(index))
        )?;
    }
    writeln!(out, "];")?;

    let cap = scroll_cap();
    let mut cap_width = 0.0_f32;
    let mut cap_height = 0.0_f32;
    for index in 0..SCROLL_CAP_POSE_COUNT {
        let phase = scroll_cap_phase(index);
        let top = scroll_cap_pose(&cap, phase, -1.0);
        let bottom = scroll_cap_pose(&cap, phase, 1.0);
        let [width, height] = projected_span(&top);
        cap_width = cap_width.max(width);
        cap_height = cap_height.max(height);
        let top = compile_scroll_cap(&top, phase, -1.0);
        let bottom = compile_scroll_cap(&bottom, phase, 1.0);
        emit_mesh(&mut out, &format!("TOP_CAP_{index:02}"), &top)?;
        emit_mesh(&mut out, &format!("BOTTOM_CAP_{index:02}"), &bottom)?;
    }
    writeln!(
        out,
        "pub(super) const CAP_WIDTH: f32 = {};",
        scalar(cap_width)
    )?;
    writeln!(
        out,
        "pub(super) const CAP_HEIGHT: f32 = {};",
        scalar(cap_height)
    )?;
    writeln!(out, "static CAP_POSES: [BakedCapPose; CAP_POSE_COUNT] = [")?;
    for index in 0..SCROLL_CAP_POSE_COUNT {
        writeln!(
            out,
            "BakedCapPose {{ phase: {}, top: TOP_CAP_{index:02}, bottom: BOTTOM_CAP_{index:02} }},",
            scalar(scroll_cap_phase(index))
        )?;
    }
    writeln!(out, "];")?;
    writeln!(
        out,
        "pub(super) static ATLAS: BakedScrewScroll = BakedScrewScroll {{ screws: &SCREW_POSES, caps: &CAP_POSES }};"
    )
}

fn bake_material_study(path: &Path) -> io::Result<()> {
    let button_model = monoglyph_plunger(MONOGLYPH_REST, momentary_gauge(MECHANISM_SIDE_LARGE));
    let plate_model = bail_plate(bail_gauge(MECHANISM_SIDE_LARGE));
    let button_shadow = compile_shadow(&button_model, 0.0, 82);
    let plate_shadow = compile_shadow(&plate_model, 0.0, 44);
    let production_button = compile_darkened_bronze(&button_model, 1.0);
    let production_plate = compile_darkened_bronze(&plate_model, 1.0);

    let mut out = BufWriter::new(File::create(path)?);
    writeln!(out, "// @generated by build/foundry_atlas.rs; do not edit.")?;
    writeln!(
        out,
        "pub(super) const ROW_COUNT: usize = {};",
        MATERIAL_STUDY_ROWS.len()
    )?;
    writeln!(
        out,
        "pub(super) const COLUMN_COUNT: usize = {};",
        MATERIAL_STUDY_EXPOSURES.len()
    )?;
    writeln!(
        out,
        "pub(super) const PRODUCTION_ROW: usize = {MATERIAL_STUDY_PRODUCTION_ROW};"
    )?;
    writeln!(
        out,
        "pub(super) const PRODUCTION_COLUMN: usize = {MATERIAL_STUDY_PRODUCTION_COLUMN};"
    )?;
    writeln!(out, "pub(super) static ROW_NAMES: [&str; ROW_COUNT] = [")?;
    for row in MATERIAL_STUDY_ROWS {
        writeln!(out, "{:?},", row.name)?;
    }
    writeln!(out, "];")?;
    writeln!(
        out,
        "pub(super) static GLINT_EXPONENTS: [f32; ROW_COUNT] = ["
    )?;
    for row in MATERIAL_STUDY_ROWS {
        writeln!(out, "{},", scalar(row.glint_shine))?;
    }
    writeln!(out, "];")?;
    writeln!(out, "pub(super) static EXPOSURES: [f32; COLUMN_COUNT] = [")?;
    for exposure in MATERIAL_STUDY_EXPOSURES {
        writeln!(out, "{},", scalar(exposure))?;
    }
    writeln!(out, "];")?;
    emit_mesh(&mut out, "BUTTON_SHADOW", &button_shadow)?;
    emit_mesh(&mut out, "PLATE_SHADOW", &plate_shadow)?;

    for (row_index, law) in MATERIAL_STUDY_ROWS.into_iter().enumerate() {
        for (column_index, exposure) in MATERIAL_STUDY_EXPOSURES.into_iter().enumerate() {
            let button = compile_bronze_with(&button_model, |vertex| {
                material_study_lit(vertex, law, exposure)
            });
            let plate = compile_bronze_with(&plate_model, |vertex| {
                material_study_lit(vertex, law, exposure)
            });
            if row_index == MATERIAL_STUDY_PRODUCTION_ROW
                && column_index == MATERIAL_STUDY_PRODUCTION_COLUMN
            {
                assert_eq!(button.vertices(), production_button.vertices());
                assert_eq!(button.indices(), production_button.indices());
                assert_eq!(plate.vertices(), production_plate.vertices());
                assert_eq!(plate.indices(), production_plate.indices());
            }
            emit_mesh(
                &mut out,
                &format!("CELL_{row_index}_{column_index}_BUTTON"),
                &button,
            )?;
            emit_mesh(
                &mut out,
                &format!("CELL_{row_index}_{column_index}_PLATE"),
                &plate,
            )?;
        }
    }
    writeln!(
        out,
        "pub(super) static CELLS: [BakedStudyCell; ROW_COUNT * COLUMN_COUNT] = ["
    )?;
    for row in 0..MATERIAL_STUDY_ROWS.len() {
        for column in 0..MATERIAL_STUDY_EXPOSURES.len() {
            writeln!(
                out,
                "BakedStudyCell {{ button: CELL_{row}_{column}_BUTTON, plate: CELL_{row}_{column}_PLATE }},"
            )?;
        }
    }
    writeln!(out, "];")
}

const CURSOR_SIDE: usize = 84;
const CURSOR_MARGIN: f32 = 2.5;
const CURSOR_SAMPLES: usize = 4;

fn bake_longinus_cursor(path: &Path) -> io::Result<()> {
    let model = longinus();
    let mesh = compile_bronze_with(&model, polished_lit);
    let hotspot = project(longinus_pose(V3::new(0.0, -29.0, 1.0)));
    let (rgba, hotspot) = raster_cursor(&mesh, hotspot);
    let mut out = BufWriter::new(File::create(path)?);
    writeln!(out, "// @generated by build/foundry_atlas.rs; do not edit.")?;
    writeln!(out, "pub(super) const SIDE: u16 = {CURSOR_SIDE};")?;
    writeln!(out, "pub(super) const HOTSPOT: [u16; 2] = {hotspot:?};")?;
    writeln!(out, "pub(super) static RGBA: &[u8] = &[")?;
    for row in rgba.chunks(CURSOR_SIDE * 4) {
        write!(out, "    ")?;
        for byte in row {
            write!(out, "{byte},")?;
        }
        writeln!(out)?;
    }
    writeln!(out, "];")
}

fn polished_lit(vertex: Vertex) -> [u8; 4] {
    let position = vertex.position;
    let normal = vertex.normal;
    let rgb = bronze_rgb(polished_metal_tone(
        [position.x, position.y, position.z],
        [normal.x, normal.y, normal.z],
    ));
    let expose = |channel: u8| (f32::from(channel) * 1.22).round().min(255.0) as u8;
    [expose(rgb[0]), expose(rgb[1]), expose(rgb[2]), 255]
}

fn raster_cursor(mesh: &Compiled, hotspot: [f32; 2]) -> (Vec<u8>, [u16; 2]) {
    let min = [
        mesh.vertices()
            .iter()
            .map(|vertex| vertex.position[0])
            .fold(f32::INFINITY, f32::min),
        mesh.vertices()
            .iter()
            .map(|vertex| vertex.position[1])
            .fold(f32::INFINITY, f32::min),
    ];
    let max = [
        mesh.vertices()
            .iter()
            .map(|vertex| vertex.position[0])
            .fold(f32::NEG_INFINITY, f32::max),
        mesh.vertices()
            .iter()
            .map(|vertex| vertex.position[1])
            .fold(f32::NEG_INFINITY, f32::max),
    ];
    let span = [max[0] - min[0], max[1] - min[1]];
    let scale = ((CURSOR_SIDE as f32 - CURSOR_MARGIN * 2.0) / span[0])
        .min((CURSOR_SIDE as f32 - CURSOR_MARGIN * 2.0) / span[1]);
    let occupied = [span[0] * scale, span[1] * scale];
    let inset = [
        (CURSOR_SIDE as f32 - occupied[0]) * 0.5,
        (CURSOR_SIDE as f32 - occupied[1]) * 0.5,
    ];
    let map = |point: [f32; 2]| {
        [
            inset[0] + (point[0] - min[0]) * scale,
            inset[1] + (point[1] - min[1]) * scale,
        ]
    };
    let vertices = mesh
        .vertices()
        .iter()
        .map(|vertex| Pixel {
            position: map(vertex.position),
            color: vertex.color,
        })
        .collect::<Vec<_>>();
    let sample_side = CURSOR_SIDE * CURSOR_SAMPLES;
    let mut samples = vec![[0_u8; 4]; sample_side * sample_side];
    for triangle in mesh.indices().chunks_exact(3) {
        let tri = [
            vertices[triangle[0] as usize],
            vertices[triangle[1] as usize],
            vertices[triangle[2] as usize],
        ];
        let min_x = tri
            .iter()
            .map(|vertex| vertex.position[0])
            .fold(f32::INFINITY, f32::min);
        let max_x = tri
            .iter()
            .map(|vertex| vertex.position[0])
            .fold(f32::NEG_INFINITY, f32::max);
        let min_y = tri
            .iter()
            .map(|vertex| vertex.position[1])
            .fold(f32::INFINITY, f32::min);
        let max_y = tri
            .iter()
            .map(|vertex| vertex.position[1])
            .fold(f32::NEG_INFINITY, f32::max);
        let x0 = (min_x * CURSOR_SAMPLES as f32).floor().max(0.0) as usize;
        let x1 = (max_x * CURSOR_SAMPLES as f32)
            .ceil()
            .min(sample_side as f32) as usize;
        let y0 = (min_y * CURSOR_SAMPLES as f32).floor().max(0.0) as usize;
        let y1 = (max_y * CURSOR_SAMPLES as f32)
            .ceil()
            .min(sample_side as f32) as usize;
        for y in y0..y1 {
            for x in x0..x1 {
                let point = [
                    (x as f32 + 0.5) / CURSOR_SAMPLES as f32,
                    (y as f32 + 0.5) / CURSOR_SAMPLES as f32,
                ];
                if let Some(weights) = barycentric(tri.map(|vertex| vertex.position), point) {
                    let mut color = [0_u8; 4];
                    for (channel, slot) in color.iter_mut().enumerate() {
                        *slot = tri
                            .iter()
                            .zip(weights)
                            .map(|(vertex, weight)| f32::from(vertex.color[channel]) * weight)
                            .sum::<f32>()
                            .round()
                            .clamp(0.0, 255.0) as u8;
                    }
                    samples[y * sample_side + x] = color;
                }
            }
        }
    }
    let mut rgba = vec![0_u8; CURSOR_SIDE * CURSOR_SIDE * 4];
    for y in 0..CURSOR_SIDE {
        for x in 0..CURSOR_SIDE {
            let mut sum = [0_u32; 4];
            let mut covered = 0_u32;
            for sy in 0..CURSOR_SAMPLES {
                for sx in 0..CURSOR_SAMPLES {
                    let sample =
                        samples[(y * CURSOR_SAMPLES + sy) * sample_side + x * CURSOR_SAMPLES + sx];
                    if sample[3] != 0 {
                        covered += 1;
                        for channel in 0..3 {
                            sum[channel] += u32::from(sample[channel]);
                        }
                    }
                }
            }
            let pixel = &mut rgba[(y * CURSOR_SIDE + x) * 4..][..4];
            for (slot, total) in pixel[..3].iter_mut().zip(&sum[..3]) {
                *slot = total.checked_div(covered).unwrap_or(0) as u8;
            }
            pixel[3] = (covered * 255 / (CURSOR_SAMPLES * CURSOR_SAMPLES) as u32) as u8;
        }
    }
    let hotspot = map(hotspot).map(|coordinate| {
        coordinate
            .round()
            .clamp(0.0, CURSOR_SIDE.saturating_sub(1) as f32) as u16
    });
    (rgba, hotspot)
}

fn barycentric(triangle: [[f32; 2]; 3], point: [f32; 2]) -> Option<[f32; 3]> {
    let [a, b, c] = triangle;
    let denominator = (b[1] - c[1]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[1] - c[1]);
    if denominator.abs() <= f32::EPSILON {
        return None;
    }
    let u = ((b[1] - c[1]) * (point[0] - c[0]) + (c[0] - b[0]) * (point[1] - c[1])) / denominator;
    let v = ((c[1] - a[1]) * (point[0] - c[0]) + (a[0] - c[0]) * (point[1] - c[1])) / denominator;
    let w = 1.0 - u - v;
    (u >= -0.001 && v >= -0.001 && w >= -0.001).then_some([u, v, w])
}

fn emit_mesh(out: &mut impl Write, name: &str, mesh: &Compiled) -> io::Result<()> {
    emit_rust_as(
        out,
        name,
        mesh,
        RustReach::Module,
        RustDialect::new("BakedVertex", "BakedMesh"),
    )
}

/// Emit the exact IEEE-754 payload rather than a lossy decimal approximation.
/// Grouped hexadecimal also keeps generated code clear of numeric-literal
/// heuristics in downstream lint configurations.
fn scalar(value: f32) -> String {
    let bits = value.to_bits();
    format!("f32::from_bits(0x{:04x}_{:04x})", bits >> 16, bits & 0xffff)
}

fn emit_shadow(out: &mut impl Write, name: &str, shadow: &Compiled) -> io::Result<()> {
    emit_mesh(out, name, shadow)?;
    writeln!(
        out,
        "pub(super) static {name}_SOURCE: BakedShadow = BakedShadow {{ mesh: {name} }};"
    )
}

fn checkbox_pose_elevation(index: usize, gauge: CheckboxGauge) -> f32 {
    match index {
        0 => gauge.pose_min,
        i if i + 1 == POSE_COUNT => gauge.pose_max,
        _ => lerp(
            gauge.pose_min,
            gauge.pose_max,
            index as f32 / (POSE_COUNT - 1) as f32,
        ),
    }
}

fn monoglyph_pose_elevation(index: usize) -> f32 {
    match index {
        0 => MONOGLYPH_POSE_MIN,
        i if i + 1 == MONOGLYPH_POSE_COUNT => MONOGLYPH_POSE_MAX,
        _ => lerp(
            MONOGLYPH_POSE_MIN,
            MONOGLYPH_POSE_MAX,
            index as f32 / (MONOGLYPH_POSE_COUNT - 1) as f32,
        ),
    }
}

fn bail_pose_angle(index: usize) -> f32 {
    match index {
        0 => BAIL_POSE_MIN,
        i if i + 1 == BAIL_POSE_COUNT => BAIL_POSE_MAX,
        _ => lerp(
            BAIL_POSE_MIN,
            BAIL_POSE_MAX,
            index as f32 / (BAIL_POSE_COUNT - 1) as f32,
        ),
    }
}

fn wheel_pose_phase(index: usize) -> f32 {
    match index {
        0 => 0.0,
        i if i + 1 == WHEEL_POSE_COUNT => WHEEL_PITCH,
        _ => WHEEL_PITCH * index as f32 / (WHEEL_POSE_COUNT - 1) as f32,
    }
}

fn scroll_screw_phase(index: usize) -> f32 {
    TAU * index as f32 / (SCROLL_SCREW_POSE_COUNT - 1) as f32
}

fn scroll_cap_phase(index: usize) -> f32 {
    let period = TAU / SCROLL_CAP_STATIONS as f32;
    period * index as f32 / (SCROLL_CAP_POSE_COUNT - 1) as f32
}

fn projected_span(model: &Model) -> [f32; 2] {
    let mut min = [f32::INFINITY; 2];
    let mut max = [f32::NEG_INFINITY; 2];
    for pixel in model
        .triangles
        .iter()
        .flatten()
        .map(|vertex| project(vertex.position))
    {
        for axis in 0..2 {
            min[axis] = min[axis].min(pixel[axis]);
            max[axis] = max[axis].max(pixel[axis]);
        }
    }
    [max[0] - min[0], max[1] - min[1]]
}

fn material_study_lit(vertex: Vertex, law: StudyReflection, exposure: f32) -> [u8; 4] {
    let position = [vertex.position.x, vertex.position.y, vertex.position.z];
    let normal = [vertex.normal.x, vertex.normal.y, vertex.normal.z];
    let (diffuse, reflection) = material_terms(position, normal);
    let tone = (DARK_AMBIENT
        + (DARK_DIFFUSE_WEIGHT * diffuse
            + law.broad_weight * reflection.powf(law.broad_shine)
            + law.glint_weight * reflection.powf(law.glint_shine)))
    .min(DARK_TONE_CEILING);
    let rgb = bronze_rgb(tone);
    let channel = |value: u8| (f32::from(value) * exposure).round().clamp(0.0, 255.0) as u8;
    [channel(rgb[0]), channel(rgb[1]), channel(rgb[2]), 255]
}

fn compile_bronze(model: &Model, exposure: f32) -> Compiled {
    forge_bronze(model, Charge::Bronze(exposure))
}

fn compile_darkened_bronze(model: &Model, exposure: f32) -> Compiled {
    forge_bronze(model, Charge::Darkened(exposure))
}

fn compile_wheel(model: &Model, phase: f32, plane: WheelPlane) -> Compiled {
    let [width, height] = plane.aperture();
    let [hx, hy] = [width * 0.5 + 0.8, height * 0.5 + 0.8];
    let mut facets = model
        .triangles
        .iter()
        .filter(|triangle| visible(triangle))
        .filter(|triangle| {
            let projected = triangle.map(|vertex| project(vertex.position));
            let min_x = projected.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min);
            let max_x = projected
                .iter()
                .map(|p| p[0])
                .fold(f32::NEG_INFINITY, f32::max);
            let min_y = projected.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min);
            let max_y = projected
                .iter()
                .map(|p| p[1])
                .fold(f32::NEG_INFINITY, f32::max);
            min_x <= hx && max_x >= -hx && min_y <= hy && max_y >= -hy
        })
        .collect::<Vec<_>>();
    facets.sort_by(|a, b| depth(a).total_cmp(&depth(b)));
    let mut compiled = Compiled::default();
    let mut visibility = HashMap::<[u32; 3], f32>::new();
    for triangle in facets {
        compiled.triangle(triangle.map(|vertex| Pixel {
            position: project(vertex.position),
            color: {
                let p = vertex.position;
                let key = [p.x.to_bits(), p.y.to_bits(), p.z.to_bits()];
                let seen = *visibility
                    .entry(key)
                    .or_insert_with(|| numerical_wheel_key_visibility(p, phase, plane));
                lit_with_key(vertex, seen)
            },
        }));
    }
    compiled
}

fn compile_scroll_cap(model: &Model, phase: f32, outward: f32) -> Compiled {
    compile_bronze_with(model, |vertex| {
        darkened_lit_with_key(
            vertex,
            scroll_cap_key_visibility(vertex.position, phase, outward),
        )
    })
}

fn compile_darkened_crown(model: &Model, elevation: f32) -> Compiled {
    compile_darkened_bronze(model, monoglyph_shade(elevation).crown)
}

fn compile_close_crown(model: &Model, elevation: f32, gauge: CloseGauge) -> Compiled {
    compile_bronze_with(model, |vertex| {
        darkened_lit_with_key(
            vertex,
            close_key_visibility(vertex.position, elevation, gauge),
        )
    })
}

fn plunger(elevation: f32, gauge: CheckboxGauge) -> Model {
    let mut model = Model::default();
    dish(&mut model, elevation, gauge);
    crown(&mut model, elevation, gauge);
    bevel(&mut model, elevation, gauge);
    skirt(&mut model, elevation, gauge);
    model
}

fn sort_pointer(elevation: f32, side: u8, ascending: bool) -> Model {
    let scale = f32::from(side) / f32::from(MECHANISM_SIDE_LARGE);
    let hx = SORT_POINTER_HALF_X * scale;
    let hy = SORT_POINTER_HALF_Y * scale;
    let depth = SORT_POINTER_DEPTH * scale;
    let orient = |point: V3| {
        if ascending {
            point
        } else {
            V3::new(-point.x, -point.y, point.z)
        }
    };
    let face = [
        orient(V3::new(0.0, -hy, elevation)),
        orient(V3::new(hx, hy * 0.72, elevation)),
        orient(V3::new(-hx, hy * 0.72, elevation)),
    ];
    let floor = face.map(|point| point - V3::new(0.0, 0.0, depth));
    let mut model = Model::default();
    let up = V3::new(0.0, 0.0, 1.0);
    let down = V3::new(0.0, 0.0, -1.0);
    model.triangle(
        Vertex::new(face[0], up),
        Vertex::new(face[1], up),
        Vertex::new(face[2], up),
    );
    model.triangle(
        Vertex::new(floor[2], down),
        Vertex::new(floor[1], down),
        Vertex::new(floor[0], down),
    );
    for edge in 0..3 {
        let next = (edge + 1) % 3;
        let span = face[next] - face[edge];
        let normal = V3::new(span.y, -span.x, 0.0).normalized();
        model.quad([
            Vertex::new(floor[edge], normal),
            Vertex::new(floor[next], normal),
            Vertex::new(face[next], normal),
            Vertex::new(face[edge], normal),
        ]);
    }
    model
}

fn monoglyph_plunger(elevation: f32, gauge: MomentaryGauge) -> Model {
    let mut model = Model::default();
    let h = gauge.top_half;
    planar_face(&mut model, [h, h], elevation);
    square_bevel(
        &mut model,
        elevation,
        gauge.top_half,
        gauge.body_half,
        MONOGLYPH_BEVEL_DEPTH,
    );
    square_skirt(
        &mut model,
        elevation,
        gauge.body_half,
        MONOGLYPH_BEVEL_DEPTH,
        MONOGLYPH_BODY_ROOT,
    );
    model
}

fn momentary_socket(gauge: MomentaryGauge) -> Model {
    const RISE: f32 = 0.42;
    const BEVEL_RUN: f32 = 0.18;
    const FACE_WIDTH: f32 = RIM_WIDTH - 2.0 * BEVEL_RUN;
    const _: () = assert!(FACE_WIDTH > 0.0);

    let outer_floor = gauge.socket_half - MOMENTARY_CASING_INSET;
    let outer_crown = outer_floor - BEVEL_RUN;
    let inner_crown = outer_crown - FACE_WIDTH;
    let inner_floor = inner_crown - BEVEL_RUN;
    let mut model = Model::default();
    square_bevel(&mut model, RISE, outer_crown, outer_floor, RISE);
    square_ring_face(&mut model, outer_crown, inner_crown, RISE);
    square_inner_bevel(&mut model, RISE, inner_crown, inner_floor);
    model
}

fn square_ring_face(model: &mut Model, outer: f32, inner: f32, elevation: f32) {
    let vertex = |x, y| Vertex::new(V3::new(x, y, elevation), V3::new(0.0, 0.0, 1.0));
    model.quad([
        vertex(-outer, -outer),
        vertex(outer, -outer),
        vertex(inner, -inner),
        vertex(-inner, -inner),
    ]);
    model.quad([
        vertex(outer, -outer),
        vertex(outer, outer),
        vertex(inner, inner),
        vertex(inner, -inner),
    ]);
    model.quad([
        vertex(outer, outer),
        vertex(-outer, outer),
        vertex(-inner, inner),
        vertex(inner, inner),
    ]);
    model.quad([
        vertex(-outer, outer),
        vertex(-outer, -outer),
        vertex(-inner, -inner),
        vertex(-inner, inner),
    ]);
}

fn square_inner_bevel(model: &mut Model, elevation: f32, crown: f32, floor: f32) {
    let vertex = |point: V3, normal: V3| Vertex::new(point, normal.normalized());
    let depth = elevation;
    model.quad([
        vertex(
            V3::new(-crown, -crown, elevation),
            V3::new(0.0, depth, crown - floor),
        ),
        vertex(
            V3::new(crown, -crown, elevation),
            V3::new(0.0, depth, crown - floor),
        ),
        vertex(
            V3::new(floor, -floor, 0.0),
            V3::new(0.0, depth, crown - floor),
        ),
        vertex(
            V3::new(-floor, -floor, 0.0),
            V3::new(0.0, depth, crown - floor),
        ),
    ]);
    model.quad([
        vertex(
            V3::new(crown, -crown, elevation),
            V3::new(-depth, 0.0, crown - floor),
        ),
        vertex(
            V3::new(crown, crown, elevation),
            V3::new(-depth, 0.0, crown - floor),
        ),
        vertex(
            V3::new(floor, floor, 0.0),
            V3::new(-depth, 0.0, crown - floor),
        ),
        vertex(
            V3::new(floor, -floor, 0.0),
            V3::new(-depth, 0.0, crown - floor),
        ),
    ]);
    model.quad([
        vertex(
            V3::new(crown, crown, elevation),
            V3::new(0.0, -depth, crown - floor),
        ),
        vertex(
            V3::new(-crown, crown, elevation),
            V3::new(0.0, -depth, crown - floor),
        ),
        vertex(
            V3::new(-floor, floor, 0.0),
            V3::new(0.0, -depth, crown - floor),
        ),
        vertex(
            V3::new(floor, floor, 0.0),
            V3::new(0.0, -depth, crown - floor),
        ),
    ]);
    model.quad([
        vertex(
            V3::new(-crown, crown, elevation),
            V3::new(depth, 0.0, crown - floor),
        ),
        vertex(
            V3::new(-crown, -crown, elevation),
            V3::new(depth, 0.0, crown - floor),
        ),
        vertex(
            V3::new(-floor, -floor, 0.0),
            V3::new(depth, 0.0, crown - floor),
        ),
        vertex(
            V3::new(-floor, floor, 0.0),
            V3::new(depth, 0.0, crown - floor),
        ),
    ]);
}

fn corner_close_plunger(elevation: f32, gauge: CloseGauge) -> Model {
    let mut model = Model::default();
    let h = gauge.plunger.top_half;
    let step = h * 2.0 / gauge.crown_cells as f32;
    let sample = |x: usize, y: usize| {
        let x = -h + x as f32 * step;
        let y = -h + y as f32 * step;
        let epsilon = step * 0.08;
        let z = close_surface(x, y, elevation, gauge);
        let dz_dx = (close_surface(x + epsilon, y, elevation, gauge)
            - close_surface(x - epsilon, y, elevation, gauge))
            / (2.0 * epsilon);
        let dz_dy = (close_surface(x, y + epsilon, elevation, gauge)
            - close_surface(x, y - epsilon, elevation, gauge))
            / (2.0 * epsilon);
        Vertex::new(V3::new(x, y, z), V3::new(-dz_dx, -dz_dy, 1.0).normalized())
    };
    for y in 0..gauge.crown_cells {
        for x in 0..gauge.crown_cells {
            model.quad([
                sample(x, y),
                sample(x + 1, y),
                sample(x + 1, y + 1),
                sample(x, y + 1),
            ]);
        }
    }
    square_bevel(
        &mut model,
        elevation,
        gauge.plunger.top_half,
        gauge.plunger.body_half,
        MONOGLYPH_BEVEL_DEPTH,
    );
    square_skirt(
        &mut model,
        elevation,
        gauge.plunger.body_half,
        MONOGLYPH_BEVEL_DEPTH,
        MONOGLYPH_BODY_ROOT,
    );
    model
}

fn close_surface(x: f32, y: f32, elevation: f32, gauge: CloseGauge) -> f32 {
    elevation - CLOSE_DENT_DEPTH * close_relief(x, y, gauge)
}

fn close_relief(x: f32, y: f32, gauge: CloseGauge) -> f32 {
    let inverse_root_two = 0.5_f32.sqrt();
    let descending = close_cut(
        (x + y) * inverse_root_two,
        (y - x) * inverse_root_two,
        gauge,
    );
    let ascending = close_cut(
        (x - y) * inverse_root_two,
        (x + y) * inverse_root_two,
        gauge,
    );
    1.0 - descending.min(ascending).clamp(0.0, 1.0)
}

fn close_cut(along: f32, across: f32, gauge: CloseGauge) -> f32 {
    let flank = (across.abs() - gauge.floor_half) / (gauge.mouth_half - gauge.floor_half);
    let cap = (along.abs() - gauge.floor_reach) / (gauge.mouth_reach - gauge.floor_reach);
    flank.max(cap)
}

fn close_key_visibility(position: V3, elevation: f32, gauge: CloseGauge) -> f32 {
    let top_half = gauge.plunger.top_half;
    let dent = elevation - position.z;
    if dent <= 0.02 || position.x.abs() > top_half || position.y.abs() > top_half {
        return 1.0;
    }

    // The key lies in the y-z plane. March a ray from the recessed surface
    // toward that light; any higher part of the stamped crown blocks it.
    let light = V3::new(0.0, LIGHT_Y, LIGHT_Z);
    let mut distance = 0.05;
    while distance < 4.0 {
        let ray = position + light * distance;
        if ray.y < -top_half {
            break;
        }
        if ray.z + 0.025 < close_surface(ray.x, ray.y, elevation, gauge) {
            return 0.0;
        }
        distance += 0.05;
    }
    1.0
}

fn dish(model: &mut Model, elevation: f32, gauge: CheckboxGauge) {
    const CELLS: usize = 8;
    let sample = |x: usize, y: usize| {
        let u = x as f32 / CELLS as f32 * 2.0 - 1.0;
        let v = y as f32 / CELLS as f32 * 2.0 - 1.0;
        let u4 = u.powi(4);
        let v4 = v.powi(4);
        let z = elevation - gauge.bowl_depth * (1.0 - u4) * (1.0 - v4);
        let dz_dx = 4.0 * gauge.bowl_depth * u.powi(3) * (1.0 - v4) / gauge.dish_half;
        let dz_dy = 4.0 * gauge.bowl_depth * v.powi(3) * (1.0 - u4) / gauge.dish_half;
        Vertex::new(
            V3::new(u * gauge.dish_half, v * gauge.dish_half, z),
            V3::new(-dz_dx, -dz_dy, 1.0).normalized(),
        )
    };
    for y in 0..CELLS {
        for x in 0..CELLS {
            model.quad([
                sample(x, y),
                sample(x + 1, y),
                sample(x + 1, y + 1),
                sample(x, y + 1),
            ]);
        }
    }
}

fn crown(model: &mut Model, elevation: f32, gauge: CheckboxGauge) {
    let z = V3::new(0.0, 0.0, 1.0);
    let v = |x, y| Vertex::new(V3::new(x, y, elevation), z);
    let h = gauge.top_half;
    let d = gauge.dish_half;
    model.quad([v(-h, -h), v(h, -h), v(d, -d), v(-d, -d)]);
    model.quad([v(h, -h), v(h, h), v(d, d), v(d, -d)]);
    model.quad([v(h, h), v(-h, h), v(-d, d), v(d, d)]);
    model.quad([v(-h, h), v(-h, -h), v(-d, -d), v(-d, d)]);
}

fn bevel(model: &mut Model, elevation: f32, gauge: CheckboxGauge) {
    square_bevel(
        model,
        elevation,
        gauge.top_half,
        gauge.body_half,
        gauge.bevel_depth,
    );
}

fn square_bevel(model: &mut Model, elevation: f32, h: f32, b: f32, depth: f32) {
    rectangular_bevel(model, elevation, [h, h], [b, b], depth);
}

fn planar_face(model: &mut Model, [hx, hy]: [f32; 2], z: f32) {
    let columns = (2.0 * hx / DARK_REFLECTION_CELL).ceil() as usize;
    let rows = (2.0 * hy / DARK_REFLECTION_CELL).ceil() as usize;
    let vertex = |x: usize, y: usize| {
        let x = -hx + 2.0 * hx * x as f32 / columns as f32;
        let y = -hy + 2.0 * hy * y as f32 / rows as f32;
        Vertex::new(V3::new(x, y, z), V3::new(0.0, 0.0, 1.0))
    };
    for y in 0..rows {
        for x in 0..columns {
            model.quad([
                vertex(x, y),
                vertex(x + 1, y),
                vertex(x + 1, y + 1),
                vertex(x, y + 1),
            ]);
        }
    }
}

fn rectangular_bevel(
    model: &mut Model,
    elevation: f32,
    [hx, hy]: [f32; 2],
    [bx, by]: [f32; 2],
    depth: f32,
) {
    let floor = elevation - depth;
    let face = |positions: [V3; 4], normal: V3, model: &mut Model| {
        model.quad(positions.map(|position| Vertex::new(position, normal.normalized())));
    };
    face(
        [
            V3::new(-hx, -hy, elevation),
            V3::new(hx, -hy, elevation),
            V3::new(bx, -by, floor),
            V3::new(-bx, -by, floor),
        ],
        V3::new(0.0, -depth, by - hy),
        model,
    );
    face(
        [
            V3::new(hx, -hy, elevation),
            V3::new(hx, hy, elevation),
            V3::new(bx, by, floor),
            V3::new(bx, -by, floor),
        ],
        V3::new(depth, 0.0, bx - hx),
        model,
    );
    face(
        [
            V3::new(hx, hy, elevation),
            V3::new(-hx, hy, elevation),
            V3::new(-bx, by, floor),
            V3::new(bx, by, floor),
        ],
        V3::new(0.0, depth, by - hy),
        model,
    );
    face(
        [
            V3::new(-hx, hy, elevation),
            V3::new(-hx, -hy, elevation),
            V3::new(-bx, -by, floor),
            V3::new(-bx, by, floor),
        ],
        V3::new(-depth, 0.0, bx - hx),
        model,
    );
}

fn skirt(model: &mut Model, elevation: f32, gauge: CheckboxGauge) {
    square_skirt(
        model,
        elevation,
        gauge.body_half,
        gauge.bevel_depth,
        gauge.body_root,
    );
}

fn square_skirt(model: &mut Model, elevation: f32, h: f32, bevel_depth: f32, root: f32) {
    let top = elevation - bevel_depth;
    let face = |positions: [V3; 4], normal: V3, model: &mut Model| {
        model.quad(positions.map(|position| Vertex::new(position, normal)));
    };
    face(
        [
            V3::new(-h, -h, top),
            V3::new(h, -h, top),
            V3::new(h, -h, root),
            V3::new(-h, -h, root),
        ],
        V3::new(0.0, -1.0, 0.0),
        model,
    );
    face(
        [
            V3::new(h, -h, top),
            V3::new(h, h, top),
            V3::new(h, h, root),
            V3::new(h, -h, root),
        ],
        V3::new(1.0, 0.0, 0.0),
        model,
    );
    face(
        [
            V3::new(h, h, top),
            V3::new(-h, h, top),
            V3::new(-h, h, root),
            V3::new(h, h, root),
        ],
        V3::new(0.0, 1.0, 0.0),
        model,
    );
    face(
        [
            V3::new(-h, h, top),
            V3::new(-h, -h, top),
            V3::new(-h, -h, root),
            V3::new(-h, h, root),
        ],
        V3::new(-1.0, 0.0, 0.0),
        model,
    );
}

fn scroll_screw(phase: f32) -> Model {
    let skirt = SCROLL_THREAD_RADIUS * 1.8;
    let mut model = tube(
        &[
            V3::new(0.0, -skirt, 0.0),
            V3::new(0.0, SCROLL_LEAD + skirt, 0.0),
        ],
        V3::new(1.0, 0.0, 0.0),
        SCROLL_SCREW_RADIUS,
        false,
    );
    let stations = SCROLL_HELIX_STEPS + 2;
    let helix = (0..=stations)
        .map(|station| {
            let winding = (station as f32 - 1.0) / SCROLL_HELIX_STEPS as f32;
            let alpha = TAU * winding;
            let (sin, cos) = (alpha + phase).sin_cos();
            V3::new(
                SCROLL_PITCH_RADIUS * cos,
                SCROLL_LEAD * winding,
                SCROLL_PITCH_RADIUS * sin,
            )
        })
        .collect::<Vec<_>>();
    let start = -TAU / SCROLL_HELIX_STEPS as f32 + phase;
    let (sin, cos) = start.sin_cos();
    model.append(tube(
        &helix,
        V3::new(cos, 0.0, sin),
        SCROLL_THREAD_RADIUS,
        false,
    ));
    model
}

fn scroll_cap() -> Model {
    let mut model = Model::default();
    for top in [true, false] {
        let center = scroll_cap_vertex(0.0, 0, top);
        let first = SCROLL_CAP_RADIUS * (FRAC_PI_2 / SCROLL_CAP_RADIAL_RINGS as f32).sin();
        for longitude in 0..SCROLL_CAP_LONGITUDES {
            let next = (longitude + 1) % SCROLL_CAP_LONGITUDES;
            let current = scroll_cap_vertex(first, longitude, top);
            let next = scroll_cap_vertex(first, next, top);
            if top {
                model.triangle(center, current, next);
            } else {
                model.triangle(center, next, current);
            }
        }
        for ring in 1..SCROLL_CAP_RADIAL_RINGS {
            let inner = SCROLL_CAP_RADIUS
                * (FRAC_PI_2 * ring as f32 / SCROLL_CAP_RADIAL_RINGS as f32).sin();
            let outer = SCROLL_CAP_RADIUS
                * (FRAC_PI_2 * (ring + 1) as f32 / SCROLL_CAP_RADIAL_RINGS as f32).sin();
            for longitude in 0..SCROLL_CAP_LONGITUDES {
                let next = (longitude + 1) % SCROLL_CAP_LONGITUDES;
                let [ic, oc, on, inn] = [
                    scroll_cap_vertex(inner, longitude, top),
                    scroll_cap_vertex(outer, longitude, top),
                    scroll_cap_vertex(outer, next, top),
                    scroll_cap_vertex(inner, next, top),
                ];
                if top {
                    model.quad([ic, oc, on, inn]);
                } else {
                    model.quad([ic, inn, on, oc]);
                }
            }
        }
    }
    for longitude in 0..SCROLL_CAP_LONGITUDES {
        let next = (longitude + 1) % SCROLL_CAP_LONGITUDES;
        let bottom = scroll_cap_side_vertex(longitude, false);
        let bottom_next = scroll_cap_side_vertex(next, false);
        let top_next = scroll_cap_side_vertex(next, true);
        let top = scroll_cap_side_vertex(longitude, true);
        model.quad([bottom, bottom_next, top_next, top]);
    }
    model
}

fn scroll_cap_vertex(radius: f32, longitude: usize, top: bool) -> Vertex {
    let theta = TAU * longitude as f32 / SCROLL_CAP_LONGITUDES as f32;
    let (sin, cos) = theta.sin_cos();
    let [x, y] = [radius * cos, radius * sin];
    if top {
        let surface = scroll_cap_surface(x, y);
        Vertex::new(V3::new(x, y, surface.z), surface.normal)
    } else {
        Vertex::new(
            V3::new(x, y, -SCROLL_CAP_HALF_DEPTH),
            V3::new(0.0, 0.0, -1.0),
        )
    }
}

fn scroll_cap_side_vertex(longitude: usize, top: bool) -> Vertex {
    let theta = TAU * longitude as f32 / SCROLL_CAP_LONGITUDES as f32;
    let (sin, cos) = theta.sin_cos();
    let [x, y] = [SCROLL_CAP_RADIUS * cos, SCROLL_CAP_RADIUS * sin];
    let z = if top {
        scroll_cap_surface(x, y).z
    } else {
        -SCROLL_CAP_HALF_DEPTH
    };
    Vertex::new(V3::new(x, y, z), V3::new(cos, sin, 0.0))
}

fn scroll_cap_surface(x: f32, y: f32) -> WheelSurface {
    let mut surface = WheelSurface {
        z: SCROLL_CAP_HALF_DEPTH,
        normal: V3::new(0.0, 0.0, 1.0),
    };
    for station in 0..SCROLL_CAP_STATIONS {
        let theta = TAU * station as f32 / SCROLL_CAP_STATIONS as f32;
        let (sin, cos) = theta.sin_cos();
        let [dx, dy] = [x - SCROLL_COVE_STATION * cos, y - SCROLL_COVE_STATION * sin];
        let under_root = SCROLL_COVE_RADIUS.powi(2) - dx * dx - dy * dy;
        if under_root <= 0.0 {
            continue;
        }
        let root = under_root.sqrt();
        let z = SCROLL_COVE_CENTER_Z - root;
        if z < surface.z {
            surface = WheelSurface {
                z,
                normal: V3::new(-dx, -dy, root).normalized(),
            };
        }
    }
    surface
}

fn scroll_cap_pose(model: &Model, phase: f32, outward: f32) -> Model {
    model.transformed(
        |point| {
            let point = point.rotate_z(phase);
            V3::new(point.x, outward * point.z, -point.y)
        },
        |normal| {
            let normal = normal.rotate_z(phase);
            V3::new(normal.x, outward * normal.z, -normal.y)
        },
    )
}

fn scroll_cap_key_visibility(position: V3, phase: f32, outward: f32) -> f32 {
    const STEP: f32 = 0.10;
    const START: f32 = 0.07;
    let light = V3::new(0.0, LIGHT_Y, LIGHT_Z);
    let mut distance = START;
    while distance <= 2.0 * SCROLL_CAP_RADIUS + 2.0 * SCROLL_CAP_HALF_DEPTH {
        let world = position + light * distance;
        let canonical = V3::new(world.x, -world.z, outward * world.y).rotate_z(-phase);
        if scroll_cap_contains(canonical) {
            return 0.0;
        }
        distance += STEP;
    }
    1.0
}

fn scroll_cap_contains(point: V3) -> bool {
    point.x * point.x + point.y * point.y <= SCROLL_CAP_RADIUS.powi(2)
        && point.z >= -SCROLL_CAP_HALF_DEPTH
        && point.z < scroll_cap_surface(point.x, point.y).z - 0.01
}

#[derive(Clone, Copy)]
struct WheelSurface {
    z: f32,
    normal: V3,
}

fn numerical_thumbwheel() -> Model {
    let mut model = Model::default();
    for side in [1.0_f32, -1.0] {
        let center = wheel_vertex(0.0, 0.0, side);
        let first_radius = WHEEL_RADIUS * (FRAC_PI_2 / WHEEL_RADIAL_RINGS as f32).sin();
        for longitude in 0..WHEEL_LONGITUDES {
            let next = (longitude + 1) % WHEEL_LONGITUDES;
            model.triangle(
                center,
                wheel_ring_vertex(first_radius, longitude, side),
                wheel_ring_vertex(first_radius, next, side),
            );
        }
        for ring in 1..WHEEL_RADIAL_RINGS {
            let inner = WHEEL_RADIUS * (FRAC_PI_2 * ring as f32 / WHEEL_RADIAL_RINGS as f32).sin();
            let outer =
                WHEEL_RADIUS * (FRAC_PI_2 * (ring + 1) as f32 / WHEEL_RADIAL_RINGS as f32).sin();
            for longitude in 0..WHEEL_LONGITUDES {
                let next = (longitude + 1) % WHEEL_LONGITUDES;
                model.quad([
                    wheel_ring_vertex(inner, longitude, side),
                    wheel_ring_vertex(outer, longitude, side),
                    wheel_ring_vertex(outer, next, side),
                    wheel_ring_vertex(inner, next, side),
                ]);
            }
        }
    }
    model
}

fn wheel_ring_vertex(radius: f32, longitude: usize, side: f32) -> Vertex {
    let theta = TAU * longitude as f32 / WHEEL_LONGITUDES as f32;
    let (sin, cos) = theta.sin_cos();
    wheel_vertex(radius * cos, radius * sin, side)
}

fn wheel_vertex(x: f32, y: f32, side: f32) -> Vertex {
    let surface = numerical_wheel_surface(x, y);
    Vertex::new(
        V3::new(x, y, side * surface.z),
        V3::new(surface.normal.x, surface.normal.y, side * surface.normal.z),
    )
}

fn numerical_wheel_surface(x: f32, y: f32) -> WheelSurface {
    let radial_fraction = (x * x + y * y) / WHEEL_RADIUS.powi(2);
    let z = WHEEL_HALF_DEPTH * (1.0 - radial_fraction).max(0.0).sqrt();
    let mut surface = WheelSurface {
        z,
        normal: V3::new(
            x / WHEEL_RADIUS.powi(2),
            y / WHEEL_RADIUS.powi(2),
            z / WHEEL_HALF_DEPTH.powi(2),
        )
        .normalized(),
    };
    for station in 0..WHEEL_STATIONS {
        let theta = TAU * station as f32 / WHEEL_STATIONS as f32;
        let (sin, cos) = theta.sin_cos();
        let radial = V3::new(cos, sin, 0.0);
        let tangent = V3::new(-sin, cos, 0.0);
        let point = V3::new(x, y, 0.0);
        let u = point.dot(tangent);
        let v = point.dot(radial) - WHEEL_SCALLOP_RADIUS;
        let cut = WHEEL_SCALLOP_VERTEX
            + WHEEL_SCALLOP_TANGENT_CURVATURE * u * u
            + WHEEL_SCALLOP_RADIAL_CURVATURE * v * v;
        if cut < surface.z {
            let gradient = tangent * (2.0 * WHEEL_SCALLOP_TANGENT_CURVATURE * u)
                + radial * (2.0 * WHEEL_SCALLOP_RADIAL_CURVATURE * v);
            surface = WheelSurface {
                z: cut,
                normal: V3::new(-gradient.x, -gradient.y, 1.0).normalized(),
            };
        }
    }
    surface
}

fn numerical_wheel_key_visibility(position: V3, phase: f32, plane: WheelPlane) -> f32 {
    const STEP: f32 = 0.12;
    const START: f32 = 0.08;
    let light = V3::new(0.0, LIGHT_Y, LIGHT_Z);
    let mut distance = START;
    while distance <= 2.0 * WHEEL_RADIUS + 2.0 * WHEEL_HALF_DEPTH {
        let world = position + light * distance;
        let oriented = match plane {
            WheelPlane::YZ => V3::new(-world.z, world.y, world.x),
            WheelPlane::XZ => V3::new(world.x, -world.z, world.y),
        };
        if numerical_wheel_contains(oriented.rotate_z(-phase)) {
            return 0.0;
        }
        distance += STEP;
    }
    1.0
}

fn numerical_wheel_contains(point: V3) -> bool {
    let radial_fraction = (point.x * point.x + point.y * point.y) / WHEEL_RADIUS.powi(2);
    if radial_fraction > 1.0 {
        return false;
    }
    let surface = numerical_wheel_surface(point.x, point.y).z;
    point.z.abs() < surface - 0.01
}

fn bail_plate(gauge: BailGauge) -> Model {
    let mut model = Model::default();
    let h = gauge.face_half;
    planar_face(&mut model, [h, h], gauge.plate_rise);
    square_bevel(
        &mut model,
        gauge.plate_rise,
        gauge.face_half,
        gauge.base_half,
        gauge.plate_rise,
    );
    model
}

fn friction_plate(gauge: FrictionGauge) -> Model {
    let mut model = Model::default();
    planar_face(
        &mut model,
        [gauge.face_half_x, gauge.face_half_y],
        gauge.plate_rise,
    );
    rectangular_bevel(
        &mut model,
        gauge.plate_rise,
        [gauge.face_half_x, gauge.face_half_y],
        [gauge.base_half_x, gauge.base_half_y],
        gauge.plate_rise,
    );
    model
}

fn bail_hardware(gauge: BailGauge) -> Model {
    let mut model = Model::default();
    hatch_plate(
        &mut model,
        gauge.face_half - 0.8,
        gauge.face_half - 0.8,
        gauge.plate_rise,
        BAIL_HATCH_PITCH,
        BAIL_HATCH_WIDTH,
        BAIL_HATCH_RISE,
    );

    for x in [-gauge.rivet_offset, gauge.rivet_offset] {
        for y in [-gauge.rivet_offset, gauge.rivet_offset] {
            model.append(sphere(V3::new(x, y, gauge.plate_rise), gauge.rivet_radius));
        }
    }

    let lug_radius = gauge.stock_radius * 1.28;
    let lug_half = gauge.stock_radius * 1.05;
    for x in [-gauge.span, gauge.span] {
        let center = V3::new(x, gauge.hinge_y, gauge.hinge_z);
        model.append(sphere(center, lug_radius));
        model.append(tube(
            &[
                center + V3::new(-lug_half, 0.0, 0.0),
                center + V3::new(lug_half, 0.0, 0.0),
            ],
            V3::new(0.0, 0.0, 1.0),
            gauge.stock_radius * 1.08,
            false,
        ));
    }
    model
}

fn friction_hardware(gauge: FrictionGauge) -> Model {
    let mut model = Model::default();
    hatch_plate(
        &mut model,
        gauge.face_half_x - 0.45,
        gauge.rivet_y - gauge.rivet_radius - FRICTION_HATCH_WIDTH,
        gauge.plate_rise,
        FRICTION_HATCH_PITCH,
        FRICTION_HATCH_WIDTH,
        FRICTION_HATCH_RISE,
    );
    for x in [-gauge.rivet_x, gauge.rivet_x] {
        for y in [-gauge.rivet_y, gauge.rivet_y] {
            model.append(sphere(V3::new(x, y, gauge.plate_rise), gauge.rivet_radius));
        }
    }

    model
}

fn hatch_plate(
    model: &mut Model,
    half_x: f32,
    half_y: f32,
    z: f32,
    pitch: f32,
    width: f32,
    rise: f32,
) {
    let stations = ((half_x + half_y) / pitch).ceil() as i32;
    for station in -stations..=stations {
        let offset = station as f32 * pitch;
        for descending in [false, true] {
            if let Some((start, end)) = hatch_segment(half_x, half_y, offset, descending) {
                model.append(ridge(start, end, z, width, rise));
            }
        }
    }
}

fn hatch_segment(half_x: f32, half_y: f32, offset: f32, descending: bool) -> Option<(V3, V3)> {
    let (y_lo, y_hi) = if descending {
        (offset - half_y, offset + half_y)
    } else {
        (-half_y - offset, half_y - offset)
    };
    let lo = (-half_x).max(y_lo);
    let hi = half_x.min(y_hi);
    (hi - lo > 0.4).then(|| {
        let y = |x: f32| if descending { -x + offset } else { x + offset };
        (V3::new(lo, y(lo), 0.0), V3::new(hi, y(hi), 0.0))
    })
}

fn ridge(start: V3, end: V3, z: f32, width: f32, rise: f32) -> Model {
    let axis = (end - start).normalized();
    let wing = V3::new(-axis.y, axis.x, 0.0);
    let start = V3::new(start.x, start.y, z);
    let end = V3::new(end.x, end.y, z);
    let left = wing * (width * 0.5);
    let apex = V3::new(0.0, 0.0, rise);
    let mut model = Model::default();
    for (base, sign) in [(left, 1.0), (left * -1.0, -1.0)] {
        let mut normal = axis.cross(apex - base).normalized() * sign;
        if normal.z < 0.0 {
            normal = normal * -1.0;
        }
        model.quad(
            [start + base, end + base, end + apex, start + apex]
                .map(|position| Vertex::new(position, normal)),
        );
    }
    model
}

fn bail(gauge: BailGauge, angle: f32) -> Model {
    let (sin, cos) = angle.sin_cos();
    let points = bail_profile(gauge)
        .into_iter()
        .map(|point| {
            V3::new(
                point.x,
                gauge.hinge_y + point.y * cos,
                gauge.hinge_z + point.y * sin,
            )
        })
        .collect::<Vec<_>>();
    tube(&points, V3::new(0.0, -sin, cos), gauge.stock_radius, false)
}

fn bail_profile(gauge: BailGauge) -> Vec<V3> {
    const BEND_STEPS: usize = 5;
    let bend = (gauge.stock_radius * 1.65).min(gauge.span * 0.28);
    let mut points = vec![
        V3::new(-gauge.span, 0.0, 0.0),
        V3::new(-gauge.span, gauge.reach - bend, 0.0),
    ];
    let left_center = V3::new(-gauge.span + bend, gauge.reach - bend, 0.0);
    for step in 1..=BEND_STEPS {
        let theta = lerp(PI, FRAC_PI_2, step as f32 / BEND_STEPS as f32);
        points.push(left_center + V3::new(theta.cos(), theta.sin(), 0.0) * bend);
    }
    points.push(V3::new(gauge.span - bend, gauge.reach, 0.0));
    let right_center = V3::new(gauge.span - bend, gauge.reach - bend, 0.0);
    for step in 1..=BEND_STEPS {
        let theta = lerp(FRAC_PI_2, 0.0, step as f32 / BEND_STEPS as f32);
        points.push(right_center + V3::new(theta.cos(), theta.sin(), 0.0) * bend);
    }
    points.push(V3::new(gauge.span, 0.0, 0.0));
    points
}

fn bail_sweep_per_radian(gauge: BailGauge) -> f32 {
    let profile = bail_profile(gauge);
    2.0 * gauge.stock_radius
        * profile
            .windows(2)
            .map(|span| {
                let radius = (span[0].y + span[1].y) * 0.5;
                radius * (span[1] - span[0]).length()
            })
            .sum::<f32>()
}

fn guard(gauge: GuardGauge) -> Model {
    let mut model = Model::default();
    for &x in gauge.wire_stations {
        let points = (0..=CURVE_STEPS)
            .map(|step| {
                let y = lerp(
                    -gauge.guard_half,
                    gauge.guard_half,
                    step as f32 / CURVE_STEPS as f32,
                );
                guard_wire_point(gauge, x, y, WIRE_LAYER)
            })
            .collect::<Vec<_>>();
        model.append(tube(&points, V3::new(1.0, 0.0, 0.0), WIRE_RADIUS, false));
    }
    for &y in gauge.wire_stations {
        let points = (0..=CURVE_STEPS)
            .map(|step| {
                let x = lerp(
                    -gauge.guard_half,
                    gauge.guard_half,
                    step as f32 / CURVE_STEPS as f32,
                );
                guard_wire_point(gauge, x, y, -WIRE_LAYER)
            })
            .collect::<Vec<_>>();
        model.append(tube(&points, V3::new(0.0, 1.0, 0.0), WIRE_RADIUS, false));
    }
    for &x in gauge.wire_stations {
        for &y in gauge.wire_stations {
            model.append(sphere(guard_surface(gauge, x, y).0, WELD_RADIUS));
        }
    }
    model.append(tube(
        &guard_frame(gauge),
        V3::new(0.0, 0.0, 1.0),
        FRAME_RADIUS,
        true,
    ));
    model
}

fn guard_surface(gauge: GuardGauge, x: f32, y: f32) -> (V3, V3) {
    let ax = x.abs();
    let ay = y.abs();
    let r = ax.max(ay) / gauge.guard_half;
    let z = gauge.guard_base + gauge.guard_rise * (1.0 - r.clamp(0.0, 1.0).powi(4));
    let slope = -4.0 * gauge.guard_rise * r.powi(3) / gauge.guard_half;
    let (dz_dx, dz_dy) = if ax >= ay {
        (slope * x.signum(), 0.0)
    } else {
        (0.0, slope * y.signum())
    };
    (V3::new(x, y, z), V3::new(-dz_dx, -dz_dy, 1.0).normalized())
}

fn guard_wire_point(gauge: GuardGauge, x: f32, y: f32, layer: f32) -> V3 {
    let (surface, normal) = guard_surface(gauge, x, y);
    let edge = (x.abs().max(y.abs()) / gauge.guard_half).clamp(0.0, 1.0);
    surface + normal * layer * (1.0 - edge.powi(8))
}

fn guard_frame(gauge: GuardGauge) -> Vec<V3> {
    const SAMPLES: usize = 40;
    const POWER: f32 = 6.0;
    (0..SAMPLES)
        .map(|sample| {
            let theta = sample as f32 / SAMPLES as f32 * TAU;
            let (sin, cos) = theta.sin_cos();
            V3::new(
                gauge.guard_half * cos.signum() * cos.abs().powf(2.0 / POWER),
                gauge.guard_half * sin.signum() * sin.abs().powf(2.0 / POWER),
                gauge.guard_base + FRAME_RADIUS,
            )
        })
        .collect()
}

fn tube(points: &[V3], seed: V3, radius: f32, closed: bool) -> Model {
    assert!(
        points.len() >= 2,
        "a physical wire requires at least two stations"
    );
    let n = points.len();
    let tangents = (0..n)
        .map(|i| {
            let prior = if i == 0 {
                if closed { points[n - 1] } else { points[0] }
            } else {
                points[i - 1]
            };
            let next = if i + 1 == n {
                if closed { points[0] } else { points[n - 1] }
            } else {
                points[i + 1]
            };
            (next - prior).normalized()
        })
        .collect::<Vec<_>>();
    let mut axes = Vec::with_capacity(n);
    let mut axis = (seed - tangents[0] * seed.dot(tangents[0])).normalized();
    for tangent in &tangents {
        let transported = axis - *tangent * axis.dot(*tangent);
        axis = if transported.length() > 0.01 {
            transported.normalized()
        } else {
            (seed - *tangent * seed.dot(*tangent)).normalized()
        };
        axes.push(axis);
    }

    let rings = points
        .iter()
        .zip(&tangents)
        .zip(&axes)
        .map(|((point, tangent), axis)| {
            let wing = tangent.cross(*axis).normalized();
            (0..TUBE_SIDES)
                .map(|side| {
                    let theta = side as f32 / TUBE_SIDES as f32 * TAU;
                    let normal = *axis * theta.cos() + wing * theta.sin();
                    Vertex::new(*point + normal * radius, normal)
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    let mut model = Model::default();
    let spans = if closed { n } else { n - 1 };
    for i in 0..spans {
        let next = (i + 1) % n;
        for side in 0..TUBE_SIDES {
            let around = (side + 1) % TUBE_SIDES;
            model.quad([
                rings[i][side],
                rings[next][side],
                rings[next][around],
                rings[i][around],
            ]);
        }
    }
    model
}

fn longinus() -> Model {
    const HELIX_STATIONS: usize = 56;
    let mut model = Model::default();
    for side in [-1.0_f32, 1.0] {
        let blade = (0..=16)
            .map(|station| {
                let t = station as f32 / 16.0;
                let y = lerp(-29.0, -7.0, t);
                let x = side * lerp(2.75, 2.65, t);
                (longinus_pose(V3::new(x, y, 1.0)), lerp(0.035, 0.88, t))
            })
            .collect::<Vec<_>>();
        model.append(longinus_blade(&blade, 0.52));

        let phase = if side < 0.0 { PI } else { 0.0 };
        let helix = (0..HELIX_STATIONS)
            .map(|station| {
                let t = station as f32 / (HELIX_STATIONS - 1) as f32;
                let ease = t * t * (3.0 - 2.0 * t);
                let radius = lerp(2.65, 0.10, ease);
                let theta = phase + 2.35 * TAU * t;
                longinus_pose(V3::new(
                    radius * theta.cos(),
                    lerp(-7.0, 3.2, t),
                    1.0 + radius * 0.55 * theta.sin(),
                ))
            })
            .collect::<Vec<_>>();
        model.append(tube(
            &helix,
            longinus_normal(V3::new(0.0, 0.0, 1.0)),
            0.66,
            false,
        ));
    }
    let shaft = [
        longinus_pose(V3::new(0.0, 3.0, 1.0)),
        longinus_pose(V3::new(0.0, 30.0, 1.0)),
    ];
    model.append(tube(
        &shaft,
        longinus_normal(V3::new(0.0, 0.0, 1.0)),
        0.72,
        false,
    ));
    model.append(sphere(shaft[0], 0.84));
    model.append(sphere(shaft[1], 0.72));
    model
}

fn longinus_blade(stations: &[(V3, f32)], half_depth: f32) -> Model {
    let mut rings = Vec::with_capacity(stations.len());
    for (index, &(center, half_width)) in stations.iter().enumerate() {
        let prior = stations[index.saturating_sub(1)].0;
        let next = stations[(index + 1).min(stations.len() - 1)].0;
        let tangent = (next - prior).normalized();
        let across = longinus_normal(V3::new(1.0, 0.0, 0.0));
        let depth = tangent.cross(across).normalized();
        rings.push([
            Vertex::new(center + across * half_width, across),
            Vertex::new(center + depth * half_depth, depth),
            Vertex::new(center - across * half_width, across * -1.0),
            Vertex::new(center - depth * half_depth, depth * -1.0),
        ]);
    }
    let mut model = Model::default();
    for span in 0..rings.len() - 1 {
        for side in 0..4 {
            let around = (side + 1) % 4;
            model.quad([
                rings[span][side],
                rings[span + 1][side],
                rings[span + 1][around],
                rings[span][around],
            ]);
        }
    }
    model
}

fn longinus_pose(point: V3) -> V3 {
    point.rotate_z(-PI / 4.0)
}

fn longinus_normal(normal: V3) -> V3 {
    normal.rotate_z(-PI / 4.0)
}

fn sphere(center: V3, radius: f32) -> Model {
    const LATITUDES: usize = 5;
    const LONGITUDES: usize = 8;
    let vertex = |latitude: usize, longitude: usize| {
        let phi = -FRAC_PI_2 + latitude as f32 / LATITUDES as f32 * FRAC_PI_2 * 2.0;
        let theta = longitude as f32 / LONGITUDES as f32 * TAU;
        let normal = V3::new(phi.cos() * theta.cos(), phi.cos() * theta.sin(), phi.sin());
        Vertex::new(center + normal * radius, normal)
    };
    let mut model = Model::default();
    for latitude in 0..LATITUDES {
        for longitude in 0..LONGITUDES {
            let around = (longitude + 1) % LONGITUDES;
            model.quad([
                vertex(latitude, longitude),
                vertex(latitude, around),
                vertex(latitude + 1, around),
                vertex(latitude + 1, longitude),
            ]);
        }
    }
    model
}

fn verify_geometry() {
    assert!((HALF_Y * HALF_Y + HALF_Z * HALF_Z - 1.0).abs() < 1e-6);
    let fixed_half = V3::new(0.0, LIGHT_Y, LIGHT_Z) + V3::new(0.0, 0.0, 1.0);
    let fixed_half = fixed_half.normalized();
    assert!((fixed_half.y - HALF_Y).abs() < 1e-6);
    assert!((fixed_half.z - HALF_Z).abs() < 1e-6);
    for side in MECHANISM_SIDES {
        let checkbox = checkbox_gauge(side);
        verify_guard_geometry(
            side,
            checkbox.guard,
            checkbox.body_half,
            checkbox.pose_max,
            checkbox.assembly_side * 0.5,
        );
        let monoglyph = momentary_gauge(side);
        verify_guard_geometry(
            side,
            monoglyph_guard_gauge(side),
            monoglyph.body_half,
            MONOGLYPH_POSE_MAX,
            f32::from(side) * 0.5,
        );
    }
    let close = close_gauge(MECHANISM_SIDE_LARGE);
    assert!((close_relief(0.0, 0.0, close) - 1.0).abs() < f32::EPSILON);
    let top_half = close.plunger.top_half;
    assert!(close_relief(top_half, 0.0, close).abs() < f32::EPSILON);
    let floor = V3::new(0.0, 0.0, MONOGLYPH_REST - CLOSE_DENT_DEPTH);
    assert!(close_key_visibility(floor, MONOGLYPH_REST, close).abs() < f32::EPSILON);
    let cut = numerical_wheel_surface(WHEEL_SCALLOP_RADIUS, 0.0);
    assert!((cut.z - WHEEL_SCALLOP_VERTEX).abs() < 1e-5);
    let uncut = WHEEL_HALF_DEPTH * (1.0 - (WHEEL_SCALLOP_RADIUS / WHEEL_RADIUS).powi(2)).sqrt();
    assert!(cut.z < uncut);
    assert!(numerical_wheel_surface(WHEEL_RADIUS, 0.0).z.abs() < 1e-5);
    let blank = numerical_thumbwheel();
    assert!(blank.triangles.iter().flatten().all(|vertex| {
        let n = vertex.normal;
        vertex.position.x.is_finite()
            && vertex.position.y.is_finite()
            && vertex.position.z.is_finite()
            && (n.length() - 1.0).abs() < 1e-4
    }));
    let canonical_span = blank
        .triangles
        .iter()
        .flatten()
        .map(|vertex| vertex.position.length())
        .fold(0.0_f32, f32::max);
    for plane in WheelPlane::ALL {
        let oriented_span = wheel_pose(&blank, 0.37, plane)
            .triangles
            .iter()
            .flatten()
            .map(|vertex| vertex.position.length())
            .fold(0.0_f32, f32::max);
        assert!((canonical_span - oriented_span).abs() < 1e-4);
    }
    // The public atlas is deliberately discrete. Keep the forge itself proven
    // across the integer continuum so reopening size policy does not require a
    // geometry rewrite.
    for side in MECHANISM_SIDE_SMALL..=MECHANISM_SIDE_LARGE {
        let gauge = bail_gauge(side);
        let profile = bail_profile(gauge);
        assert_eq!(profile.first().map(|point| point.y), Some(0.0));
        assert_eq!(profile.last().map(|point| point.y), Some(0.0));
        let bar_floor = gauge.hinge_z + gauge.reach * BAIL_REST.sin() - gauge.stock_radius;
        assert!(
            bar_floor > gauge.plate_rise,
            "resting bail collides with its backplate at gauge {side}"
        );
        assert!(bail_sweep_per_radian(gauge) > 100.0);

        let gauge = friction_gauge(side);
        assert!(gauge.base_half_y < momentary_gauge(side).socket_half);
        assert!(gauge.face_half_x > gauge.rivet_x + gauge.rivet_radius);
        assert!(gauge.face_half_y > gauge.rivet_y + gauge.rivet_radius);
        assert!(gauge.rivet_x > gauge.rivet_radius);
        assert!(gauge.rivet_y > gauge.rivet_radius);
        let crest = friction_hardware(gauge)
            .triangles
            .iter()
            .flatten()
            .map(|vertex| vertex.position.z)
            .fold(f32::NEG_INFINITY, f32::max);
        assert!(crest > gauge.plate_rise + FRICTION_HATCH_RISE * 0.9);
        assert!(crest <= gauge.plate_rise + gauge.rivet_radius + 1e-5);
    }
}

fn verify_guard_geometry(
    side: u8,
    guard: GuardGauge,
    body_half: f32,
    pose_max: f32,
    footprint_half: f32,
) {
    assert!(guard.guard_half + FRAME_RADIUS <= footprint_half);
    assert_eq!(
        guard.wire_stations.len(),
        match side {
            MECHANISM_SIDE_SMALL => 2,
            MECHANISM_SIDE_MEDIUM => 3,
            MECHANISM_SIDE_LARGE => 4,
            _ => unreachable!(),
        }
    );
    assert!(
        guard
            .wire_stations
            .windows(2)
            .all(|stations| (stations[1] - stations[0] - 7.0).abs() < f32::EPSILON)
    );
    for &x in guard.wire_stations {
        for step in 0..=CURVE_STEPS {
            let y = lerp(
                -guard.guard_half,
                guard.guard_half,
                step as f32 / CURVE_STEPS as f32,
            );
            let wire = guard_wire_point(guard, x, y, WIRE_LAYER);
            if x.abs() <= body_half && y.abs() <= body_half {
                assert!(
                    wire.z - WIRE_RADIUS > pose_max,
                    "upper guard wire collides with gauge {side} crown at ({x}, {y})"
                );
            }
        }
    }
    for &y in guard.wire_stations {
        for step in 0..=CURVE_STEPS {
            let x = lerp(
                -guard.guard_half,
                guard.guard_half,
                step as f32 / CURVE_STEPS as f32,
            );
            let wire = guard_wire_point(guard, x, y, -WIRE_LAYER);
            if x.abs() <= body_half && y.abs() <= body_half {
                assert!(
                    wire.z - WIRE_RADIUS > pose_max,
                    "lower guard wire collides with gauge {side} crown at ({x}, {y})"
                );
            }
        }
    }
    for &x in guard.wire_stations {
        for &y in guard.wire_stations {
            let upper = guard_wire_point(guard, x, y, WIRE_LAYER);
            let lower = guard_wire_point(guard, x, y, -WIRE_LAYER);
            let edge = x.abs().max(y.abs()) / guard.guard_half;
            let separation = 2.0 * WIRE_LAYER * (1.0 - edge.powi(8));
            assert!(((upper - lower).length() - separation).abs() < 1e-5);
            assert!((upper - lower).length() <= 2.0 * WELD_RADIUS);
        }
    }
}

fn lerp(lo: f32, hi: f32, t: f32) -> f32 {
    lo + (hi - lo) * t
}

//! Poolrooms gauges adjoining the public Brass Foundry material law.

pub(crate) use brass_foundry::*;

pub(crate) const MECHANISM_SIDE_SMALL: u8 = 20;
pub(crate) const MECHANISM_SIDE_MEDIUM: u8 = 24;
pub(crate) const MECHANISM_SIDE_LARGE: u8 = 32;
pub(crate) const RIM_WIDTH: f32 = 1.0;
pub(crate) const MOMENTARY_CASING_INSET: f32 = RIM_WIDTH;
pub(crate) const MONOGLYPH_REST: f32 = 3.25;
pub(crate) const MONOGLYPH_LATCH: f32 = -4.85;
pub(crate) const MONOGLYPH_PRESS: f32 = -7.15;
#[allow(
    dead_code,
    reason = "the complete bake roster is consumed only by the build-time geometry compiler"
)]
pub(crate) const MECHANISM_SIDES: [u8; 3] = [
    MECHANISM_SIDE_SMALL,
    MECHANISM_SIDE_MEDIUM,
    MECHANISM_SIDE_LARGE,
];

/// X-y dimensions of one momentary mechanism gauge. Z travel and cutting-tool
/// depths remain common foundry stock, so changing the footprint changes the
/// bevel normals rather than resampling a finished projection.
#[derive(Clone, Copy)]
pub(crate) struct MomentaryGauge {
    pub(crate) socket_half: f32,
    pub(crate) top_half: f32,
    pub(crate) body_half: f32,
}

pub(crate) const fn momentary_gauge(side: u8) -> MomentaryGauge {
    let side = side as f32;
    let socket_half = side * 0.5;
    MomentaryGauge {
        socket_half,
        top_half: socket_half * (89.0 / 132.0),
        body_half: socket_half * (49.0 / 66.0),
    }
}

/// Exposure register for one monoglyph elevation. The samples are deliberately
/// finite: socket occlusion inspires the levels, but the Foundry owns their
/// legible screen projection rather than pretending to solve indirect light.
#[derive(Clone, Copy)]
pub(crate) struct MonoglyphShade {
    pub(crate) crown: f32,
    pub(crate) bright_cut: f32,
    pub(crate) void: f32,
    pub(crate) danger: f32,
    pub(crate) love: f32,
}

const RAISED_SHADE: MonoglyphShade = MonoglyphShade {
    crown: 1.0,
    bright_cut: 1.0,
    void: 1.0,
    danger: 1.0,
    love: 1.0,
};
const LATCHED_SHADE: MonoglyphShade = MonoglyphShade {
    crown: 0.68,
    bright_cut: 0.74,
    void: 0.78,
    danger: 0.63,
    love: 0.67,
};
const PRESSED_SHADE: MonoglyphShade = MonoglyphShade {
    crown: 0.54,
    bright_cut: 0.62,
    void: 0.70,
    danger: 0.51,
    love: 0.56,
};

impl MonoglyphShade {
    fn mix(self, other: Self, t: f32) -> Self {
        let mix = |a, b| a + (b - a) * t;
        Self {
            crown: mix(self.crown, other.crown),
            bright_cut: mix(self.bright_cut, other.bright_cut),
            void: mix(self.void, other.void),
            danger: mix(self.danger, other.danger),
            love: mix(self.love, other.love),
        }
    }
}

/// Resolve the tabulated raised, latched, and pressed exposures at one physical
/// crown elevation.
pub(crate) fn monoglyph_shade(elevation: f32) -> MonoglyphShade {
    if elevation >= MONOGLYPH_LATCH {
        let t = ((MONOGLYPH_REST - elevation) / (MONOGLYPH_REST - MONOGLYPH_LATCH)).clamp(0.0, 1.0);
        RAISED_SHADE.mix(LATCHED_SHADE, t)
    } else {
        let t =
            ((MONOGLYPH_LATCH - elevation) / (MONOGLYPH_LATCH - MONOGLYPH_PRESS)).clamp(0.0, 1.0);
        LATCHED_SHADE.mix(PRESSED_SHADE, t)
    }
}

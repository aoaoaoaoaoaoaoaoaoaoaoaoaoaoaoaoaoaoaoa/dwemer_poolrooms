// CPU/WGSL treaty shared verbatim by the compute and optical stages.
struct Chemistry {
    reach: f32,
    meniscus_px: f32,
    refract_px: f32,
    ior_spread: f32,
    quiver_bulge: f32,
    quiver_pulse: f32,
    tremor_k: f32,
    tremor_omega: f32,
    tremor_amp: f32,
    tremor_fade: f32,
    tremor_reach: f32,
    bulge_px: f32,
    lift_bright: f32,
    wave_v: f32,
    wave_sigma: f32,
    wave_damp: f32,
    wave_spread: f32,
    source_gain: f32,
    height_retention: f32,
    tilt_gain: f32,
    t_panel: f32,
    r_panel: f32,
    r_wall: f32,
    shore_feather: f32,
}

struct Forcing {
    cut_rects: array<vec4f, 2>,
    cut_vitals: array<vec4f, 2>,
    domain: vec4f,
    optics: vec4f,
    motion: vec4f,
    lift_rects: array<vec4f, 4>,
    lift_grips: vec4f,
    quivers: array<Quiver, 4>,
    splashes: array<Splash, 32>,
    pond: vec4f,
    touches: array<Touch, 12>,
    chemistry: Chemistry,
    raft_rect: vec4f,
    raft_corners: vec4f,
    floor_rect: vec4f,
    floor_vitals: vec4f,
}

struct Quiver { rect: vec4f, touch: vec4f }

struct Splash { rect: vec4f, vitals: vec4f }

struct Touch { wave: vec4f }

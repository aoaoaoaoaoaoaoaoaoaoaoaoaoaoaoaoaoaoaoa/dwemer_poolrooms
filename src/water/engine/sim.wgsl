override SIM_SCALE: f32 = 2.0;
override DT: f32 = 1.0 / 240.0;
override IMPULSE_GAIN: f32 = 1.0;
const LIFT_RADIUS: f32 = 3.0;
const PLATE_FEATHER: f32 = 6.0;
const SOURCE_LIFE: f32 = 0.22;
const BASIN_TAPER: f32 = 28.0;
const BASIN_GAIN: f32 = 0.5;
const JITTER_GAIN: f32 = 2.0; // broadband thwack strength (noise is ±0.5)
const SOURCE_CEIL: f32 = 72.0;
const SOURCE_KICK_CEIL: f32 = 96.0;
const H_CEIL: f32 = 48.0;
const V_CEIL: f32 = 1440.0;
const TILT_FORCE_CEIL: f32 = 48.0;
const RAFT_STIFFNESS: f32 = 420.0;
const KO_EPSILON: f32 = 0.018;
const KO_SHOCK_GAIN: f32 = 10.0;
const RAIL_DAMPING: f32 = 0.52;

@group(0) @binding(0) var src_tex: texture_2d<f32>;
@group(0) @binding(1) var dst_tex: texture_storage_2d<rg32float, write>;
@group(0) @binding(2) var<uniform> forcing: Forcing;

fn cell_px(p: vec2i) -> vec2f {
    return (vec2f(p) + vec2f(0.5)) * SIM_SCALE;
}

fn sd_cut(px: vec2f, rect_min: vec2f, rect_max: vec2f, radius: f32) -> f32 {
    let center = (rect_min + rect_max) * 0.5;
    let half_size = (rect_max - rect_min) * 0.5 - radius;
    let q = abs(px - center) - half_size;
    return length(max(q, vec2f(0.0))) + min(max(q.x, q.y), 0.0) - radius;
}

fn island(sd: f32) -> f32 { return 1.0 - smoothstep(-PLATE_FEATHER, PLATE_FEATHER, sd); }

fn finite(x: f32) -> bool { return x == x && abs(x) < 1e20; }

fn sane(x: f32, ceil: f32) -> f32 { return clamp(select(0.0, x, finite(x)), -ceil, ceil); }

fn soft_limiter(x: f32, ceil: f32) -> f32 { return ceil * x / (abs(x) + ceil); }

fn rail_damp(h: f32, v: f32) -> f32 {
    let strain = max(abs(h) / H_CEIL, abs(v) / V_CEIL);
    return mix(1.0, RAIL_DAMPING, smoothstep(0.62, 0.92, strain));
}

fn quiver_omega(q: Quiver) -> f32 { return select(forcing.chemistry.tremor_omega, q.touch.w, q.touch.w > 0.0); }

fn domain_gate(px: vec2f) -> f32 {
    let d = sd_cut(px, forcing.domain.xy, forcing.domain.zw, 0.0);
    return 1.0 - smoothstep(-PLATE_FEATHER, 0.0, d);
}

fn obstacle(px: vec2f) -> f32 {
    var block = forcing.motion.y * (1.0 - domain_gate(px));
    for (var i = 0u; i < 2u; i = i + 1u) {
        let cut = forcing.cut_rects[i];
        block = max(
            block,
            island(sd_cut(px, cut.xy, cut.zw, forcing.cut_vitals[i].x))
                * forcing.cut_vitals[i].y,
        );
    }
    for (var i = 0u; i < 4u; i = i + 1u) {
        let g = abs(forcing.lift_grips[i]);
        if (g <= 0.0) {
            continue;
        }
        let r = forcing.lift_rects[i];
        block = max(block, island(sd_cut(px, r.xy, r.zw, LIFT_RADIUS)) * g);
    }
    return clamp(block, 0.0, 1.0);
}

fn load_state(p: vec2i, dims: vec2i) -> vec2f {
    let raw = textureLoad(src_tex, clamp(p, vec2i(0), dims - vec2i(1)), 0).xy;
    return vec2f(sane(raw.x, H_CEIL), sane(raw.y, V_CEIL));
}

fn wall_height(p: vec2i, dims: vec2i, h: f32) -> f32 {
    let q = clamp(p, vec2i(0), dims - vec2i(1));
    let b = obstacle(cell_px(q));
    return mix(load_state(q, dims).x, h, b);
}

fn source_shell(px: vec2f, rect: vec4f, age: f32, amp: f32) -> f32 {
    if (amp == 0.0 || age > SOURCE_LIFE) {
        return 0.0;
    }
    let d = sd_cut(px, rect.xy, rect.zw, LIFT_RADIUS);
    let hull = smoothstep(-PLATE_FEATHER, 0.0, d);
    let shell = exp(-0.5 * pow(max(d, 0.0) / max(forcing.chemistry.wave_sigma, 1.0), 2.0));
    let birth = 1.0 - smoothstep(0.0, SOURCE_LIFE, age);
    return amp * hull * shell * birth;
}

fn source_basin(px: vec2f, rect: vec4f, age: f32, amp: f32) -> f32 {
    if (amp == 0.0 || age > SOURCE_LIFE) {
        return 0.0;
    }
    let d = sd_cut(px, rect.xy, rect.zw, LIFT_RADIUS);
    let taper = max(BASIN_TAPER, forcing.chemistry.wave_sigma * 1.6);
    let prism = 1.0 - smoothstep(-taper, 0.0, d);
    let birth = 1.0 - smoothstep(0.0, SOURCE_LIFE, age);
    return amp * prism * birth * BASIN_GAIN;
}

// A solid sweeping water along one screen axis. Friction piles the bulk off the
// leading edge with weaker trailing suction: the bow-wave/wake dipole of its
// displaced volume. `along` is the driven coordinate and `across` its span.
fn source_sweep(
    along: f32,
    across: f32,
    lo: f32,
    hi: f32,
    cross_center: f32,
    cross_half: f32,
    age: f32,
    amp: f32,
    travel: f32,
) -> f32 {
    if (amp == 0.0 || age > SOURCE_LIFE) {
        return 0.0;
    }
    let reach = max(forcing.chemistry.wave_sigma * 1.2, 8.0);
    let breadth = exp(-0.5 * (across - cross_center) * (across - cross_center) / (cross_half * cross_half));
    let lead = select(lo, hi, travel > 0.0) + travel * reach;
    let bulge = exp(-0.5 * (along - lead) * (along - lead) / (reach * reach));
    let trail = select(hi, lo, travel > 0.0) - travel * reach * 0.5;
    let suck = exp(-0.5 * (along - trail) * (along - trail) / (reach * reach * 1.6));
    let birth = 1.0 - smoothstep(0.0, SOURCE_LIFE, age);
    return amp * birth * breadth * (bulge - 0.5 * suck);
}

fn source_drag(px: vec2f, rect: vec4f, age: f32, amp: f32, travel: f32) -> f32 {
    let cx = (rect.x + rect.z) * 0.5;
    let half_w = max((rect.z - rect.x) * 0.5, 4.0);
    return source_sweep(px.y, px.x, rect.y, rect.w, cx, half_w, age, amp, travel);
}

fn source_slide(px: vec2f, rect: vec4f, age: f32, amp: f32, travel: f32) -> f32 {
    let cy = (rect.y + rect.w) * 0.5;
    let half_h = max((rect.w - rect.y) * 0.5, 4.0);
    return source_sweep(px.x, px.y, rect.x, rect.z, cy, half_h, age, amp, travel);
}

fn hash12(p: vec2f) -> f32 {
    return fract(sin(dot(p, vec2f(127.1, 311.7))) * 43758.5453);
}

// The whole sheet thwacked from beneath: zero-mean broadband velocity noise, one
// value per cell (Nyquist content). The KO term shreds the high-k within a few
// steps — a quick shimmer — while the uniform friction carries the sparse low-k
// residue out. Flat noise is broadband, so the solver does the spectral shaping.
fn source_jitter(px: vec2f, rect: vec4f, age: f32, amp: f32) -> f32 {
    if (amp == 0.0 || age > SOURCE_LIFE) {
        return 0.0;
    }
    let inside = clamp(-sd_cut(px, rect.xy, rect.zw, 0.0) / PLATE_FEATHER, 0.0, 1.0);
    let birth = 1.0 - smoothstep(0.0, SOURCE_LIFE, age);
    // Re-seed the field each frame off the tide, so it boils rather than
    // freezing into one fixed pattern when thwacks pile up on a fast scroll.
    // (Velocity noise integrates into height, so the surface still reads smooth.)
    let seed = px + fract(forcing.optics.w) * vec2f(53.0, 97.0);
    return amp * inside * birth * JITTER_GAIN * (hash12(seed) - 0.5);
}

fn source(px: vec2f) -> f32 {
    var drive = 0.0;
    for (var i = 0u; i < 32u; i = i + 1u) {
        let splash = forcing.splashes[i];
        if (splash.vitals.z > 2.5) { drive = drive + source_slide(px, splash.rect, splash.vitals.x, splash.vitals.y, splash.vitals.w); }
        else if (abs(splash.vitals.w) > 0.5) { drive = drive + source_drag(px, splash.rect, splash.vitals.x, splash.vitals.y, splash.vitals.w); }
        else if (splash.vitals.z > 1.5) { drive = drive + source_jitter(px, splash.rect, splash.vitals.x, splash.vitals.y); }
        else if (splash.vitals.z > 0.5) { drive = drive + source_basin(px, splash.rect, splash.vitals.x, splash.vitals.y); }
        else { drive = drive + source_shell(px, splash.rect, splash.vitals.x, splash.vitals.y); }
    }
    for (var i = 0u; i < 4u; i = i + 1u) {
        let q = forcing.quivers[i];
        let g = q.touch.z;
        if (g <= 0.0) {
            continue;
        }
        let d = sd_cut(px, q.rect.xy, q.rect.zw, LIFT_RADIUS);
        if (d <= 0.0 || d > forcing.chemistry.tremor_reach) {
            continue;
        }
        let shell = exp(-d / max(forcing.chemistry.tremor_fade, 1.0));
        let phase = forcing.chemistry.tremor_k * d - quiver_omega(q) * forcing.optics.w;
        drive = drive + forcing.chemistry.tremor_amp * g * shell * sin(phase);
    }
    return soft_limiter(drive, SOURCE_CEIL);
}

fn tilt_drive(px: vec2f) -> f32 {
    let span = max(forcing.domain.w - forcing.domain.y, 1.0);
    let y = clamp((px.y - forcing.domain.y) / span, 0.0, 1.0);
    let ramp = y * 2.0 - 1.0;
    let lip = max(SIM_SCALE * 2.0, 1.0);
    let y_gate = smoothstep(forcing.domain.y, forcing.domain.y + lip, px.y)
        * (1.0 - smoothstep(forcing.domain.w - lip, forcing.domain.w, px.y));
    let shelf_gate = smoothstep(
        forcing.domain.x - forcing.chemistry.shore_feather,
        forcing.domain.x + forcing.chemistry.shore_feather,
        px.x,
    );
    let x_gate = mix(shelf_gate, domain_gate(px), forcing.motion.y);
    let force = -clamp(forcing.motion.x, -TILT_FORCE_CEIL, TILT_FORCE_CEIL) * ramp;
    return force * max(forcing.chemistry.tilt_gain, 0.0) * x_gate * y_gate;
}

fn raft_height(px: vec2f) -> f32 {
    let r = forcing.raft_rect;
    let size = r.zw - r.xy;
    if (size.x <= 1.0 || size.y <= 1.0) {
        return 0.0;
    }
    let uv = clamp((px - r.xy) / size, vec2f(0.0), vec2f(1.0));
    let top = mix(forcing.raft_corners.x, forcing.raft_corners.y, uv.x);
    let bottom = mix(forcing.raft_corners.w, forcing.raft_corners.z, uv.x);
    let sheet = mix(top, bottom, uv.y);
    let gate = island(sd_cut(px, r.xy, r.zw, LIFT_RADIUS));
    return sheet * gate;
}

fn raft_drive(px: vec2f, h: f32) -> f32 {
    return (raft_height(px) - h) * RAFT_STIFFNESS;
}

@compute @workgroup_size(8, 8, 1)
fn step(@builtin(global_invocation_id) gid: vec3u) {
    let dims_u = textureDimensions(src_tex);
    if (gid.x >= dims_u.x || gid.y >= dims_u.y) {
        return;
    }
    let dims = vec2i(dims_u);
    let p = vec2i(gid.xy);
    let px = cell_px(p);
    let here = load_state(p, dims);
    let h = here.x;

    let l = wall_height(p + vec2i(-1, 0), dims, h);
    let r = wall_height(p + vec2i(1, 0), dims, h);
    let u = wall_height(p + vec2i(0, -1), dims, h);
    let d = wall_height(p + vec2i(0, 1), dims, h);
    let lap = (l + r + u + d - 4.0 * h) / (SIM_SCALE * SIM_SCALE);
    let lu = wall_height(p + vec2i(-1, -1), dims, h);
    let ru = wall_height(p + vec2i(1, -1), dims, h);
    let ld = wall_height(p + vec2i(-1, 1), dims, h);
    let rd = wall_height(p + vec2i(1, 1), dims, h);
    let ll = wall_height(p + vec2i(-2, 0), dims, h);
    let rr = wall_height(p + vec2i(2, 0), dims, h);
    let uu = wall_height(p + vec2i(0, -2), dims, h);
    let dd = wall_height(p + vec2i(0, 2), dims, h);
    let rough = clamp(abs(l + r + u + d - 4.0 * h) / (4.0 * H_CEIL), 0.0, 1.0);
    let ko_gain = KO_EPSILON * mix(1.0, KO_SHOCK_GAIN, smoothstep(0.18, 0.72, rough));
    let ko = (
        20.0 * h
            - 8.0 * (l + r + u + d)
            + 2.0 * (lu + ru + ld + rd)
            + ll + rr + uu + dd
    ) * (ko_gain / 64.0);

    let shelf = mix(
        smoothstep(
            -forcing.chemistry.shore_feather,
            forcing.chemistry.shore_feather,
            px.x - forcing.domain.x,
        ),
        1.0,
        forcing.motion.y,
    );
    let shelf_speed = clamp((1.0 - forcing.chemistry.r_panel) / (1.0 + forcing.chemistry.r_panel), 0.2, 1.0);
    let cfl = 0.66 * SIM_SCALE / DT;
    let c = min(forcing.chemistry.wave_v * mix(shelf_speed, 1.0, shelf), cfl);
    let kick = soft_limiter(source(px) * forcing.chemistry.source_gain * IMPULSE_GAIN, SOURCE_KICK_CEIL);
    var v = here.y
        + c * c * lap * DT
        + kick
        + (tilt_drive(px) + raft_drive(px, h)) * DT;
    v = v * exp(-DT / max(forcing.chemistry.wave_damp, 0.08)) * mix(0.985, 1.0, shelf);
    v = v * rail_damp(h, v);

    let block = obstacle(px);
    v = mix(v, 0.0, block);
    let keep = clamp(forcing.chemistry.height_retention, 0.95, 1.0);
    var next_h = mix((h + v * DT) * keep - ko, h * keep, block);
    v = sane(v, V_CEIL);
    next_h = sane(next_h, H_CEIL);
    textureStore(dst_tex, p, vec4f(next_h, v, 0.0, 0.0));
}

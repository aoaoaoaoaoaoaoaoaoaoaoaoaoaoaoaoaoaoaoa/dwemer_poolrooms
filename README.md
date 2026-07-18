# Dwemer Poolrooms

Dwemer Poolrooms is the reusable visual and physical substrate extracted from
`adequate_booru_viewer`: its embedded fonts, dark bronze egui chrome, responsive
control plates, veil optics, and persistent GPU water table.

It is deliberately one crate with two strata:

- `chrome` is renderer-agnostic egui style and widgets. It is always available.
- `water` owns forcing history, oscillators, damping, scheduling, shader packing,
  and the egui-wgpu compositor. It is the default feature.

Applications own meaning and geometry. They may say “this rectangle was struck”
or place a raw excitation anywhere; they cannot manufacture shader packets,
manage source ages, or depend on the field's capacity.

## Use

Until the crate is published, depend on it by path or Git. Import egui through
the crate so its public geometry types cannot drift from the renderer version:

```toml
[dependencies]
dwemer_poolrooms = { path = "../dwemer_poolrooms" }
```

```rust
use dwemer_poolrooms::{chrome, egui};

let ctx = egui::Context::default();
chrome::install(&ctx);
```

For a small login or tray application that needs only the design language:

```toml
dwemer_poolrooms = { path = "../dwemer_poolrooms", default-features = false }
```

## Forcing

`WaterTable` is the application-facing physics boundary. All coordinates passed
to it are logical egui points; physical-pixel conversion happens exactly once in
`WaterTable::frame`.

```rust
use dwemer_poolrooms::water::{Poke, WaterTable, Wetness};

let mut water = WaterTable::new(Wetness::Wet);
water.begin_surface(gallery_rect);
water.hover(post_id, tile_rect);               // relaxing lift plate
water.click(tile_rect);                        // calibrated semantic strike
water.poke(tile_rect, Poke::ring(3.25));       // arbitrary positive impulse
water.poke(gutter_rect, Poke::basin(-0.8));    // arbitrary sink
water.poke(reel_rect, Poke::drag(0.4, -19.0)); // directional shove
```

The convenience vocabulary (`bump`, `click`, `lever`, `drag`, `select`,
`thwack`, `fold`, `text`, loading/drain machinery, and a bounded touch pond) is
calibrated to the house style. `poke` is the escape hatch: its placement,
amplitude, sign, and source law are consumer-controlled, while wetness scaling,
retirement, prioritization, and GPU limits remain private.

Call `begin_surface` once per UI pass. After painting, seal the pass:

```rust
let frame = water.frame(&ctx, pixels_per_point, &tooltip_rects, veil);
```

`Frame` is opaque. It exposes only whether water is live and whether another
paint is required.

## Render Graph

The water distorts the already-rasterized UI, so an eframe paint callback is too
late. A water-bearing application needs an egui-wgpu render graph with this
order:

1. Construct `Frost::new(device, surface_format)` and call `resize` in physical
   pixels whenever the surface changes.
2. If `frame.live()`, render egui into `frost.scene_view()`; if dry, render egui
   directly into the swapchain and call `clear_water`.
3. Call `frost.compose(..., swapchain_view, &frame)` in the same command encoder.
4. Submit, then call `frost.after_submit(device, queue, &frame)`.
5. Request another redraw when either `frame.wants_repaint()` or
   `after_submit` returns true.

`dwemer_poolrooms::egui_wgpu` is re-exported so the host uses the exact wgpu
type universe expected by `Frost`. Window creation, tray behavior, event-loop
wakeups, and surface recovery remain application responsibilities.

## Features

| Feature | Default | Effect |
| --- | --- | --- |
| `water` | yes | GPU field, compositor, forcing runtime, and egui-wgpu re-export |
| `instrumentation` | no | Semantic chrome anchors for deterministic UI choreography |

The shaders and fonts are embedded in the crate. CMU Typewriter is distributed
under the SIL Open Font License; the bundled Noto fonts use Apache-2.0. Their
license texts remain beside the assets.

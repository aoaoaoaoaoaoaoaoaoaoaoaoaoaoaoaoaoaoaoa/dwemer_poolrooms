# Dwemer Poolrooms

Skeuomorphic controls and living water for egui.

Poolrooms supplies embedded typography, machined bronze chrome, mechanically
constrained controls, and a persistent GPU water surface that reacts to UI
motion. Its custom controls are currently the linkage-driven [`Rail`] and
tape-transport [`DateSpool`]. Both use the same material and lighting model.

## Try It

```sh
cargo run --example slider_gallery
cargo run --example date_spool_gallery
```

## Use It

```toml
[dependencies]
dwemer_poolrooms = "0.3.1"
```

Import egui through the crate to keep its public geometry types aligned with
the renderer, then install the chrome once:

```rust
use dwemer_poolrooms::{chrome, egui};

let ctx = egui::Context::default();
chrome::install(&ctx);
```

For chrome without GPU water:

```toml
dwemer_poolrooms = { version = "0.3.1", default-features = false }
```

## Water

Water is a post-process over the already-rasterized interface. It therefore
requires a direct egui-wgpu render graph; an eframe paint callback is too late.

1. Record geometry and interaction against a `Surface` during the UI pass.
2. Render egui into `Engine::scene_view()` while the surface is live.
3. Call `Engine::compose()` into the swapchain before submitting.
4. After submission, call `Engine::after_submit()` and honor its repaint request.

[`examples/support/mod.rs`](examples/support/mod.rs) is a complete minimal host,
including input, resize, surface recovery, and repaint scheduling. `egui_wgpu`
is re-exported so consumers use the exact wgpu type universe expected by the
engine.

The default `water` feature contains the simulator and compositor.
`instrumentation` adds semantic chrome anchors for deterministic UI driving.

[`Rail`]: https://docs.rs/dwemer_poolrooms/latest/dwemer_poolrooms/chrome/struct.Rail.html
[`DateSpool`]: https://docs.rs/dwemer_poolrooms/latest/dwemer_poolrooms/chrome/struct.DateSpool.html

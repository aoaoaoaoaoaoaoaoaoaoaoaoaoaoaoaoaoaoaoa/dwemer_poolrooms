#![cfg_attr(
    test,
    expect(
        unused_crate_dependencies,
        reason = "egui-winit belongs to the dev-only live gallery host"
    )
)]
#![doc = include_str!("../README.md")]

pub mod chrome;
mod tide;

#[cfg(feature = "water")]
pub mod water;

#[cfg(feature = "instrumentation")]
pub mod instrumentation;

pub use egui;

#[cfg(feature = "water")]
pub use egui_wgpu;

/// Register a named widget rectangle for an external deterministic UI driver.
/// The entire expression disappears when instrumentation is disabled.
#[macro_export]
macro_rules! poolroom_anchor {
    ($ui:expr, $name:expr, $rect:expr) => {{
        #[cfg(feature = "instrumentation")]
        $crate::instrumentation::record($ui, $name, $rect);
    }};
}

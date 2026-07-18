//! Persistent GPU water and the forcing vocabulary that drives it.

mod gpu;
mod machines;
mod table;

pub use gpu::{Brine as Chemistry, Frost};
pub use table::{Agitation, Cut, Frame, Poke, Veil, WaterTable, Wetness};

pub const BULGE_CEIL: f32 = gpu::BULGE_CEIL;

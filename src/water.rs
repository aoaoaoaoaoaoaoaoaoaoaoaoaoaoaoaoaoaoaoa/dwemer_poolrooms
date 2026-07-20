//! Persistent GPU water worlds and the forcing vocabulary that drives them.

mod engine;
mod machines;
mod surface;

pub use engine::{Chemistry, Engine};
pub use surface::{Agitation, Cut, Domain, Floor, Frame, Poke, Surface, Veil, Wetness};

pub const BULGE_CEIL: f32 = engine::BULGE_CEIL;

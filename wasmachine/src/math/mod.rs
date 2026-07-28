//! Math types: one type per physical meaning, physics-typed operators,
//! explicit conversions only. Everything is 64-bit.
//!
//! The module is split for readability but re-exports every type flat, so
//! `wasmachine::math::*` (and a plugin SDK's re-export of it) is one namespace:
//!
//! - [`vectors`] — the macro-generated vector family plus their extras
//!   (`floor`/`round`/`ceil`, `length`/`dot`/`cross`/`normalize`).
//! - [`kernel`] — the host-routed scalar primitives ([`sin`], [`pow`],
//!   [`format_f64`], …), the one part of this module that is not pure guest
//!   arithmetic.
//! - [`ticks`] — [`Ticks`], the game-tick duration.
//! - [`angle`] — [`Degrees`] and [`Radians`].
//! - [`rotation`] — [`Rotation`], the quaternion orientation.

mod angle;
mod kernel;
mod rotation;
mod ticks;
mod vectors;

pub use angle::{Degrees, Radians};
pub use kernel::{
    SHORTEST, acos, asin, atan2, cbrt, cos, exp, format_f64, ln, log, log10, pow, sin, tan,
};
pub use rotation::Rotation;
pub use ticks::Ticks;
pub use vectors::{Offset, Position, Scale, Vector3d, Vector3i, Velocity};

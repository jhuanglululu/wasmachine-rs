//! The math kernel: transcendentals and number formatting, routed to the host.
//!
//! On wasm these are the engine's imports; on the host target they are Rust's
//! own `f64` methods (see `abi::stubs`). Callers never see the difference —
//! this crate's own maths, an SDK's, and an animation's all just call
//! [`sin`], [`pow`], … and get the same numbers on both targets.
//!
//! **Why a kernel at all:** a transcendental compiled into the module is a
//! software routine costing ~500–1000 interpreted instructions per call, while
//! a host crossing costs tens — and the host's `StrictMath` is bit-identical
//! across machines, which is what makes a recorded animation trace replayable
//! anywhere. **Why not everything:** `sqrt`, `abs`, `floor`, `ceil`, `trunc`
//! and `round` are native wasm opcodes, so they are *cheaper* inline than
//! crossing; call `f64::sqrt` and friends directly, there is deliberately no
//! wrapper here.
//!
//! Domain errors follow the host (NaN propagation, ±inf) — nothing here kills.
//! The one exception is [`format_f64`]'s precision argument, which is API
//! misuse rather than a domain value.

use crate::abi;

/// Cube root, including negative inputs: `cbrt(-8.0) == -2.0`.
///
/// Kept separate from [`pow`] on purpose — `pow(x, 1.0/3.0)` is NaN for
/// negative `x`, and `1.0/3.0` is not exactly a third anyway.
pub fn cbrt(x: f64) -> f64 {
    unsafe { abi::cbrt(x) }
}

/// `x` raised to `y`, both `f64` (Rust's `powf`).
///
/// There is no `powi` kernel: an integer power is a short multiply loop, and
/// crossing to the host would cost more than computing it.
pub fn pow(x: f64, y: f64) -> f64 {
    unsafe { abi::pow(x, y) }
}

/// `e^x`.
pub fn exp(x: f64) -> f64 {
    unsafe { abi::exp(x) }
}

/// Natural logarithm.
pub fn ln(x: f64) -> f64 {
    unsafe { abi::ln(x) }
}

/// Base-10 logarithm. Its own kernel entry because the host's `log10` is more
/// accurate than `ln(x) / ln(10)`.
pub fn log10(x: f64) -> f64 {
    unsafe { abi::log10(x) }
}

/// Logarithm of `x` in an arbitrary `base` — SDK sugar, not a kernel entry:
/// `ln(x) / ln(base)`, with exact fast paths for base 10 and base *e* so the
/// common cases keep the host's own accuracy and cost one crossing instead of
/// two.
pub fn log(x: f64, base: f64) -> f64 {
    if base == 10.0 {
        log10(x)
    } else if base == core::f64::consts::E {
        ln(x)
    } else {
        ln(x) / ln(base)
    }
}

/// Sine of an angle in radians.
pub fn sin(x: f64) -> f64 {
    unsafe { abi::sin(x) }
}

/// Cosine of an angle in radians.
pub fn cos(x: f64) -> f64 {
    unsafe { abi::cos(x) }
}

/// Tangent of an angle in radians.
pub fn tan(x: f64) -> f64 {
    unsafe { abi::tan(x) }
}

/// Arc sine, in radians.
pub fn asin(x: f64) -> f64 {
    unsafe { abi::asin(x) }
}

/// Arc cosine, in radians.
pub fn acos(x: f64) -> f64 {
    unsafe { abi::acos(x) }
}

/// The angle in radians from the +X axis to the point `(x, y)` — note the
/// argument order, `y` first, as everywhere else this function exists.
pub fn atan2(y: f64, x: f64) -> f64 {
    unsafe { abi::atan2(y, x) }
}

/// Format `x` the way the host does — the one formatting an animation should
/// use for numbers it shows.
///
/// `precision` is [`SHORTEST`] (`-1`) for the shortest text that reads back as
/// exactly this `f64`, or `0..=17` for that many decimals, rounded half-to-even
/// on the exact binary value. Plain notation always: never an exponent, however
/// large or small. Negative zero keeps its sign (`-0`, `-0.00`), and a
/// non-finite value formats as `NaN`, `inf` or `-inf` whatever the precision.
///
/// A precision outside `-1..=17` is a bug in the caller and kills the animation.
///
/// **Why cross the ABI for this:** Rust's own float formatting is ~10–20 KB of
/// machinery in *every* animation and thousands of instructions per call, in a
/// domain where text displays show numbers constantly. Integer formatting is
/// trivial and stays guest-side — `{}` on an integer pulls none of that in.
///
/// ```ignore
/// text.set(format!("speed: {}", math::format_f64(v, 2)));
/// ```
pub fn format_f64(x: f64, precision: i32) -> String {
    assert!(
        (SHORTEST..=17).contains(&precision),
        "format_f64 precision must be -1 (shortest round-trip) or 0..=17, got {precision}"
    );
    abi::marshal::format_f64(x, precision)
}

/// The [`format_f64`] precision that asks for the shortest round-trip form.
pub const SHORTEST: i32 = -1;

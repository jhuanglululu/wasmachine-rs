//! The math kernel's guest-side API: known answers for the primitives, the
//! arbitrary-base `log` sugar, and `format_f64`'s contract around the pinned
//! vector table (`format_vectors.rs`).
//!
//! Expected values are mathematical facts written out by hand, not results of
//! the same call under test — on the host target these route to Rust's `f64`
//! methods, so asserting "it equals `x.sin()`" would assert nothing.

use wasmachine::math::{
    SHORTEST, acos, asin, atan2, cbrt, cos, exp, format_f64, ln, log, log10, pow, sin, tan,
};

const PI: f64 = core::f64::consts::PI;
const E: f64 = core::f64::consts::E;

fn approx(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-12
}

#[test]
fn trigonometry_at_the_landmark_angles() {
    assert_eq!(sin(0.0), 0.0);
    assert!(approx(sin(PI / 2.0), 1.0));
    assert!(approx(sin(PI), 0.0));
    assert_eq!(cos(0.0), 1.0);
    assert!(approx(cos(PI), -1.0));
    assert!(approx(tan(PI / 4.0), 1.0));
    // Sign is carried through, not folded away.
    assert!(approx(sin(-PI / 2.0), -1.0));
}

#[test]
fn inverse_trigonometry_and_atan2_quadrants() {
    assert_eq!(asin(0.0), 0.0);
    assert!(approx(asin(1.0), PI / 2.0));
    assert!(approx(acos(0.0), PI / 2.0));
    assert_eq!(acos(1.0), 0.0);

    // atan2 takes y first, and reads the quadrant from both signs.
    assert!(approx(atan2(0.0, 1.0), 0.0));
    assert!(approx(atan2(1.0, 0.0), PI / 2.0));
    assert!(approx(atan2(1.0, 1.0), PI / 4.0));
    assert!(approx(atan2(-1.0, -1.0), -3.0 * PI / 4.0));
    assert!(approx(atan2(1.0, -1.0), 3.0 * PI / 4.0));
}

/// The reason `cbrt` is its own kernel entry instead of `pow(x, 1.0/3.0)`:
/// negative inputs have a real cube root, and `pow` gives NaN for them.
#[test]
fn cbrt_handles_negative_inputs_where_pow_cannot() {
    assert_eq!(cbrt(8.0), 2.0);
    assert_eq!(cbrt(-8.0), -2.0);
    assert_eq!(cbrt(0.0), 0.0);
    assert!(pow(-8.0, 1.0 / 3.0).is_nan());
}

#[test]
fn powers_and_logarithms() {
    assert_eq!(pow(2.0, 10.0), 1024.0);
    assert_eq!(pow(9.0, 0.5), 3.0);
    assert_eq!(pow(2.0, -1.0), 0.5);
    assert_eq!(pow(0.0, 0.0), 1.0);

    assert_eq!(exp(0.0), 1.0);
    assert!(approx(exp(1.0), E));
    assert_eq!(ln(1.0), 0.0);
    assert!(approx(ln(E), 1.0));
    assert_eq!(log10(1000.0), 3.0);
    assert_eq!(log10(1.0), 0.0);
}

/// `log(x, base)` is sugar over the kernel, and its two fast paths must be
/// *exactly* the dedicated calls — not merely close — because that is the
/// accuracy the dedicated entries exist for.
#[test]
fn arbitrary_base_log_uses_exact_fast_paths() {
    for x in [0.5, 1.0, 2.0, 7.0, 1000.0, 1e-8, 1e12] {
        assert_eq!(log(x, 10.0), log10(x), "base 10 must be log10({x}) exactly");
        assert_eq!(log(x, E), ln(x), "base e must be ln({x}) exactly");
    }
    // The general path: an arbitrary base is a ratio of natural logs.
    assert!(approx(log(8.0, 2.0), 3.0));
    assert!(approx(log(81.0, 3.0), 4.0));
    assert_eq!(log(1.0, 7.0), 0.0);
}

#[test]
fn domain_errors_propagate_rather_than_kill() {
    // The kernel never kills; a caller that needs a finite answer checks.
    assert!(ln(-1.0).is_nan());
    assert!(asin(2.0).is_nan());
    assert!(ln(0.0).is_infinite() && ln(0.0) < 0.0);
    assert!(exp(1e6).is_infinite());
    assert!(sin(f64::NAN).is_nan());
}

#[test]
fn format_f64_spans_the_precision_range() {
    assert_eq!(format_f64(1.0 / 3.0, 0), "0");
    assert_eq!(format_f64(1.0 / 3.0, 5), "0.33333");
    assert_eq!(format_f64(2.0, SHORTEST), "2");
    // Both ends of the fixed range are legal.
    assert_eq!(format_f64(1.0, 0), "1");
    assert_eq!(format_f64(1.0, 17).len(), 19);
}

/// A precision outside the ABI's range is API misuse, and misuse kills — the
/// same contract the host applies to a bad argument.
#[test]
#[should_panic(expected = "precision must be -1")]
fn a_precision_above_the_range_kills() {
    let _ = format_f64(1.0, 18);
}

#[test]
#[should_panic(expected = "precision must be -1")]
fn a_precision_below_shortest_kills() {
    let _ = format_f64(1.0, -2);
}

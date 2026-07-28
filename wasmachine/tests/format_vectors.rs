//! **Shared fixture — the Java repo embeds this exact table.**
//!
//! `format_f64` is one function implemented twice: natively here (Rust's own
//! formatting, which *is* the canonical semantics) and in Java with
//! `BigDecimal` half-even rounding. Neither side is free to drift, because an
//! animation's text is compared byte for byte across machines and across
//! replays. So the agreement is pinned by vectors rather than by prose — the
//! same discipline the SplitMix64 stream gets.
//!
//! Anything failing here is a genuine cross-language conflict: fix it on
//! *both* sides, never by editing one column of the table.
//!
//! The cases were chosen for what they pin down:
//! - ties at `.5` — half-**even**, not half-up (`2.5 -> 2`, `3.5 -> 4`)
//! - values that are not what they look like (`1.005` is really
//!   `1.00499999…`, so it rounds *down*)
//! - negative zero keeping its sign, at every precision
//! - plain notation always, however large or small — no exponent ever
//! - non-finite values ignoring the precision entirely
//! - `0.3` at 17 decimals, where the exact binary value shows through

use wasmachine::math::{SHORTEST, format_f64};

/// `(x, precision, expected)`. Keep this list in sync with the Java side.
fn vectors() -> Vec<(f64, i32, String)> {
    let mut v: Vec<(f64, i32, String)> = vec![
        // Ties round half to even.
        (2.5, 0, "2".into()),
        (3.5, 0, "4".into()),
        (-2.5, 0, "-2".into()),
        // Exact binary values: 0.125 and 0.375 really are ties; 1.005 is not.
        (0.125, 2, "0.12".into()),
        (0.375, 2, "0.38".into()),
        (1.005, 2, "1.00".into()),
        (0.25, 1, "0.2".into()),
        (0.75, 1, "0.8".into()),
        // Negative zero keeps its sign.
        (-0.0, 2, "-0.00".into()),
        (-0.0, SHORTEST, "-0".into()),
        // Shortest round-trip: no trailing ".0", no exponent.
        (1.0, SHORTEST, "1".into()),
        (0.1, SHORTEST, "0.1".into()),
        (1.5, SHORTEST, "1.5".into()),
        (1e7, SHORTEST, "10000000".into()),
        (1.5e-7, SHORTEST, "0.00000015".into()),
    ];
    // 1e300 in plain notation: 301 characters, and the one case that outgrows
    // the marshalling layer's inline buffer and takes the retry path.
    v.push((1e300, SHORTEST, format!("1{}", "0".repeat(300))));
    v.extend([
        // Non-finite ignores the precision.
        (f64::NAN, 3, "NaN".to_owned()),
        (f64::INFINITY, SHORTEST, "inf".to_owned()),
        (f64::NEG_INFINITY, 0, "-inf".to_owned()),
        // Seventeen decimals of 0.3 expose the exact binary value.
        (0.3, 17, "0.29999999999999999".to_owned()),
        // --- Added after the Java side measured what its own rounding needed
        // pinning. Same rule: fix a failure on both sides, never in one column.
        //
        // 2.675 is the classic "looks like a tie, isn't" case: the double is
        // 2.674999999999999822…, so half-even has nothing to break and it
        // rounds down.
        (2.675, 2, "2.67".to_owned()),
        // Zero and whole numbers still get their decimals.
        (0.0, 2, "0.00".to_owned()),
        (-1.0, 2, "-1.00".to_owned()),
        // Shortest round-trip keeps every significant digit and adds none.
        (12345.6789, SHORTEST, "12345.6789".to_owned()),
        (100.0, SHORTEST, "100".to_owned()),
        // Seventeen decimals of an exact value: all zeros, no drift.
        (1.0, 17, "1.00000000000000000".to_owned()),
    ]);
    // The small-magnitude mirror of 1e300: 302 characters ("0." + 299 zeros +
    // the 1 in the 300th decimal place), and the other case that outgrows the
    // marshalling layer's inline buffer.
    v.push((1e-300, SHORTEST, format!("0.{}1", "0".repeat(299))));
    v
}

#[test]
fn the_pinned_cross_language_vectors() {
    for (x, precision, expected) in vectors() {
        let got = format_f64(x, precision);
        assert_eq!(
            got, expected,
            "format_f64({x:e}, {precision}) disagrees with the shared table"
        );
    }
}

/// The table's own arithmetic, checked independently of the implementation:
/// the two long cases really are the lengths claimed, so a silently truncated
/// buffer could not pass the test above by accident.
#[test]
fn the_long_vectors_are_as_long_as_they_claim() {
    let big = format_f64(1e300, SHORTEST);
    assert_eq!(
        big.len(),
        301,
        "1e300 must print as 1 followed by 300 zeros"
    );
    assert!(big.starts_with('1') && big[1..].bytes().all(|b| b == b'0'));

    // "0." + 299 zeros + "1" — one character longer than 1e300's form, because
    // the leading "0." is two characters and the exponent costs the same 300.
    let small = format_f64(1e-300, SHORTEST);
    assert_eq!(small.len(), 302, "1e-300 must print in plain notation");
    assert!(small.starts_with("0.") && small.ends_with('1'));
    assert!(small[2..small.len() - 1].bytes().all(|b| b == b'0'));

    assert_eq!(format_f64(0.3, 17).len(), 19); // "0." + 17 decimals
}

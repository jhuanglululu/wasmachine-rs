//! Host-target stubs so the crate's pure logic is unit-testable with plain
//! `cargo test`. Anything that would actually cross the boundary panics —
//! except the math kernel, which is *computed* here: it is a pure function of
//! its arguments, so a native stand-in (Rust's own `f64` methods, and Rust's own
//! formatting for [`format_f64`]) keeps `math` testable end to end. Those
//! results are also the canonical ones the Java kernel is written to match:
//! StrictMath for the transcendentals, half-even `BigDecimal` for the
//! formatter, pinned from both sides by shared test vectors.
//!
//! Compiled only for non-wasm targets.

// realloc/fail are referenced only from wasm-gated code (allocator, panic
// hook), so they're dead on the host target by design.
#![allow(dead_code, clippy::missing_safety_doc)]

pub unsafe fn realloc(_: *mut u8, _: usize, _: usize, _: usize) -> *mut u8 {
    unreachable!("wasmachine ABI called outside wasm")
}
pub unsafe fn spawn(_: i32, _: i32) -> i32 {
    unimplemented!("wasmachine ABI: spawn is wasm-only")
}
pub unsafe fn join(_: i32) {
    unimplemented!("wasmachine ABI: join is wasm-only")
}
pub unsafe fn kill(_: i32) {
    unimplemented!("wasmachine ABI: kill is wasm-only")
}
pub unsafe fn exit() -> ! {
    unimplemented!("wasmachine ABI: exit is wasm-only")
}
pub unsafe fn sleep(_: i64) {
    unimplemented!("wasmachine ABI: sleep is wasm-only")
}
pub unsafe fn log(_: *const u8, _: usize) {
    unimplemented!("wasmachine ABI: log is wasm-only")
}
pub unsafe fn fail(_: *const u8, _: usize) -> ! {
    panic!("wasmachine ABI: fail called outside wasm")
}

// --- Sync primitives. ---
pub unsafe fn signal_new() -> i32 {
    unimplemented!("wasmachine ABI: signal_new is wasm-only")
}
pub unsafe fn signal_notify(_: i32, _: i32) {
    unimplemented!("wasmachine ABI: signal_notify is wasm-only")
}
pub unsafe fn barrier_new(_: i32) -> i32 {
    unimplemented!("wasmachine ABI: barrier_new is wasm-only")
}
pub unsafe fn wait_all(_: i32, _: i32) -> i32 {
    unimplemented!("wasmachine ABI: wait_all is wasm-only")
}
pub unsafe fn wait_any(_: i32, _: i32) -> i32 {
    unimplemented!("wasmachine ABI: wait_any is wasm-only")
}
pub unsafe fn wait(_: i32) {
    unimplemented!("wasmachine ABI: wait is wasm-only")
}
pub unsafe fn channel_new(_: i32) -> i32 {
    unimplemented!("wasmachine ABI: channel_new is wasm-only")
}
pub unsafe fn channel_send(_: i32, _: *const u8, _: usize) {
    unimplemented!("wasmachine ABI: channel_send is wasm-only")
}
pub unsafe fn channel_recv_len(_: i32) -> i32 {
    unimplemented!("wasmachine ABI: channel_recv_len is wasm-only")
}
pub unsafe fn channel_recv(_: i32, _: *mut u8) {
    unimplemented!("wasmachine ABI: channel_recv is wasm-only")
}
pub unsafe fn channel_peek_len(_: i32) -> i32 {
    unimplemented!("wasmachine ABI: channel_peek_len is wasm-only")
}
pub unsafe fn channel_peek(_: i32, _: *mut u8) {
    unimplemented!("wasmachine ABI: channel_peek is wasm-only")
}
pub unsafe fn channel_try_len(_: i32) -> i32 {
    unimplemented!("wasmachine ABI: channel_try_len is wasm-only")
}
pub unsafe fn channel_clear(_: i32) {
    unimplemented!("wasmachine ABI: channel_clear is wasm-only")
}

// --- Randomness. `SplitRng` is pure guest Rust and needs none of
// these, so the pure random logic stays testable on the host. ---
pub unsafe fn random_nondet() -> i64 {
    unimplemented!("wasmachine ABI: random_nondet is wasm-only")
}
pub unsafe fn random_det() -> i64 {
    unimplemented!("wasmachine ABI: random_det is wasm-only")
}
pub unsafe fn seed_random(_: i64) {
    unimplemented!("wasmachine ABI: seed_random is wasm-only")
}

// --- The environment. Off wasm there is no host to serve a blob, so
// `crate::env` skips these entirely and reports an empty environment; the
// blob *parser* is pure guest Rust and stays unit-testable. ---
pub unsafe fn environ_len() -> i32 {
    unimplemented!("wasmachine ABI: environ_len is wasm-only")
}
pub unsafe fn environ_read(_: *mut u8) {
    unimplemented!("wasmachine ABI: environ_read is wasm-only")
}

// --- The math kernel, computed natively. `sqrt`/`abs`/rounding have no kernel
// entry (native wasm opcodes), so they have no stub either. ---
pub unsafe fn cbrt(x: f64) -> f64 {
    x.cbrt()
}
pub unsafe fn pow(x: f64, y: f64) -> f64 {
    x.powf(y)
}
pub unsafe fn exp(x: f64) -> f64 {
    x.exp()
}
pub unsafe fn ln(x: f64) -> f64 {
    x.ln()
}
pub unsafe fn log10(x: f64) -> f64 {
    x.log10()
}
pub unsafe fn sin(x: f64) -> f64 {
    x.sin()
}
pub unsafe fn cos(x: f64) -> f64 {
    x.cos()
}
pub unsafe fn tan(x: f64) -> f64 {
    x.tan()
}
pub unsafe fn asin(x: f64) -> f64 {
    x.asin()
}
pub unsafe fn acos(x: f64) -> f64 {
    x.acos()
}
pub unsafe fn atan2(y: f64, x: f64) -> f64 {
    y.atan2(x)
}

/// The canonical `format_f64` semantics, which are *defined* as Rust's own
/// formatting — `{}` for shortest round-trip (plain notation, never an
/// exponent, `-0` for negative zero) and `{:.p}` for fixed decimals (half-even
/// on the exact binary value, so `-0.00` and `1.005 -> "1.00"` survive). The
/// Java kernel mirrors it with `BigDecimal`, pinned by shared vectors.
///
/// Non-finite values ignore the precision entirely: `NaN`, `inf`, `-inf`.
/// A precision outside `-1..=17` is API misuse and kills, matching what the
/// real host does with a bad argument.
pub unsafe fn format_f64(x: f64, precision: i32, buf: *mut u8, cap: i32) -> i32 {
    let text = if x.is_finite() {
        match precision {
            -1 => format!("{x}"),
            p @ 0..=17 => format!("{x:.*}", p as usize),
            p => panic!("format_f64 precision must be -1 (shortest round-trip) or 0..=17, got {p}"),
        }
    } else {
        // NaN / inf / -inf: `{}` already prints exactly these three spellings.
        format!("{x}")
    };
    let cap = usize::try_from(cap).expect("format_f64 called with a negative capacity");
    let writable = text.len().min(cap);
    if writable > 0 {
        unsafe { core::ptr::copy_nonoverlapping(text.as_ptr(), buf, writable) };
    }
    i32::try_from(text.len()).expect("formatted f64 longer than i32::MAX bytes")
}

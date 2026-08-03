//! The real guest ABI: the engine-owned host functions, imported from module
//! `"engine"`. Compiled only for `wasm32`; see the parent module for the
//! contract.
//!
//! **Why its own module name:** while the engine and its first plugin shared
//! one namespace, every plugin feature was a potential engine edit. With
//! `"engine"` owned here and domain modules (entities, effects, …) owned by
//! plugins, the boundary is structural — the engine never learns about plugin
//! features. Guests handshake with `_engine_abi`; plugins add their own
//! handshake export beside it.

#[link(wasm_import_module = "engine")]
unsafe extern "C" {
    pub fn realloc(ptr: *mut u8, old_size: usize, align: usize, new_size: usize) -> *mut u8;
    /// Start a task running `entry(data)`. `entry` is a function-table index of
    /// an `extern "C" fn(i32)`, `data` an opaque guest pointer the host passes
    /// straight through. The host allocates the child a stack region out of the
    /// one shared heap and points its `__stack_pointer` at the top, so guests
    /// must export that global (`-C link-arg=--export=__stack_pointer`).
    /// Returns the new task id.
    pub fn spawn(entry: i32, data: i32) -> i32;
    pub fn join(task: i32);
    pub fn kill(task: i32);
    pub fn exit() -> !;
    pub fn sleep(ticks: i64);
    pub fn log(ptr: *const u8, len: usize);
    pub fn fail(ptr: *const u8, len: usize) -> !;

    // --- Sync primitives. One host-side id space covers signals, barriers,
    // composites and channels; a wrong-kind op kills. Ids are plain integers,
    // so a handle is trivially copyable into any task. ---
    pub fn signal_new() -> i32;
    pub fn signal_notify(id: i32, mode: i32);
    pub fn barrier_new(n: i32) -> i32;
    pub fn wait_all(a: i32, b: i32) -> i32;
    pub fn wait_any(a: i32, b: i32) -> i32;
    pub fn wait(id: i32);
    pub fn channel_new(cap: i32) -> i32;
    pub fn channel_send(id: i32, ptr: *const u8, len: usize);
    pub fn channel_recv_len(id: i32) -> i32;
    pub fn channel_recv(id: i32, buf: *mut u8);
    pub fn channel_peek_len(id: i32) -> i32;
    pub fn channel_peek(id: i32, buf: *mut u8);
    pub fn channel_try_len(id: i32) -> i32;
    pub fn channel_clear(id: i32);

    // --- Randomness. Two host streams (non-deterministic, and the per-instance
    // deterministic one) plus its reseed. ---
    pub fn random_nondet() -> i64;
    pub fn random_det() -> i64;
    pub fn seed_random(seed: i64);

    // --- The read-only environment: the len/fill pair serving one blob of
    // sorted key/value strings. See `crate::env` for the byte layout. ---
    pub fn environ_len() -> i32;
    pub fn environ_read(buf: *mut u8);

    // --- The math kernel. Transcendentals compile to software routines costing
    // ~500–1000 interpreted instructions each, while a host call costs tens;
    // plain arithmetic and `f64.sqrt`/`f64.abs`/`f64.floor`/`f64.ceil`/
    // `f64.trunc`/`f64.nearest` are native wasm opcodes and deliberately have no
    // kernel. Host-side these are StrictMath-backed, so results are
    // bit-identical across machines. Domain errors follow StrictMath (NaN
    // propagation, ±inf) — the kernel never kills; callers that require
    // finiteness assert it guest-side. ---
    pub fn cbrt(x: f64) -> f64;
    pub fn pow(x: f64, y: f64) -> f64;
    pub fn exp(x: f64) -> f64;
    pub fn ln(x: f64) -> f64;
    pub fn log10(x: f64) -> f64;
    pub fn sin(x: f64) -> f64;
    pub fn cos(x: f64) -> f64;
    pub fn tan(x: f64) -> f64;
    pub fn asin(x: f64) -> f64;
    pub fn acos(x: f64) -> f64;
    pub fn atan2(y: f64, x: f64) -> f64;
    /// Returns how many bytes the text needs and writes `min(needed, cap)` of
    /// them, so a short buffer is a retry rather than an error. Nothing parks
    /// between the two calls, so the retry is race-free.
    pub fn format_f64(x: f64, precision: i32, buf: *mut u8, cap: i32) -> i32;
}

//! The real guest ABI: the engine-owned host functions.
//! Compiled only for `wasm32`; see the parent module for the contract.
//!
//! The import module is still `"billboard"`: the engine and its first plugin
//! shared one namespace before this crate was extracted, and the extraction
//! deliberately moved code only — the wire ABI is unchanged, so an animation
//! built before it still loads. The namespace split (module `"engine"`,
//! `_engine_main`) is its own coordinated ABI bump, host and guest together.

#[link(wasm_import_module = "billboard")]
unsafe extern "C" {
    pub fn realloc(ptr: *mut u8, old_size: usize, align: usize, new_size: usize) -> *mut u8;
    pub fn fork() -> i32;
    pub fn join(task: i32);
    pub fn kill(task: i32);
    pub fn exit() -> !;
    pub fn sleep(ticks: i64);
    pub fn log(ptr: *const u8, len: usize);
    pub fn fail(ptr: *const u8, len: usize) -> !;

    // --- ABI v2: sync primitives. One host-side id space covers signals,
    // barriers, composites and channels; a wrong-kind op kills. Ids are plain
    // integers in the copied memory, so they survive fork for free. ---
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

    // --- ABI v2: randomness. Two host streams (non-deterministic, and the
    // per-instance deterministic one) plus its reseed. ---
    pub fn random_nondet() -> i64;
    pub fn random_det() -> i64;
    pub fn seed_random(seed: i64);
}

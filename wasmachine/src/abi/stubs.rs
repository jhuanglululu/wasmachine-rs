//! Host-target stubs so the crate's pure logic is unit-testable with plain
//! `cargo test`. Anything that would actually cross the boundary panics.
//! Compiled only for non-wasm targets.

// realloc/fail are referenced only from wasm-gated code (allocator, panic
// hook), so they're dead on the host target by design.
#![allow(dead_code, clippy::missing_safety_doc)]

pub unsafe fn realloc(_: *mut u8, _: usize, _: usize, _: usize) -> *mut u8 {
    unreachable!("wasmachine ABI called outside wasm")
}
pub unsafe fn fork() -> i32 {
    unimplemented!("wasmachine ABI: fork is wasm-only")
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

// --- ABI v2: sync primitives. ---
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

// --- ABI v2: randomness. `SplitRng` is pure guest Rust and needs none of
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

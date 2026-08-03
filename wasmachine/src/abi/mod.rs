//! The guest ABI boundary. This module — and only this module — is where raw
//! pointers and `extern` functions exist; everything above it is safe Rust.
//!
//! Contract: `context/designs/guest-abi.md` in the WASMachine repo. Only wasm
//! core types cross; strings pass as (ptr, len) into the guest's linear memory,
//! which every task of the instance shares. Task ids are i32 host handles; all
//! math crosses as f64/i64. Getters write into out-pointers; a
//! `get_*_len`/`get_*` pair has no blocking point between its two calls, so it
//! is race-free.
//!
//! The split:
//! - `sys` — the imports themselves: `wasm.rs` declares the real
//!   `unsafe extern` block on wasm; `stubs.rs` stands in on the host target so
//!   the crate's pure logic is testable with plain `cargo test`, and anything
//!   that would actually cross the boundary panics.
//! - [`marshal`] — safe wrappers for every import that takes a pointer, so that
//!   no module outside `abi` ever forms one. Callers pass `&str`/`&[u8]` and get
//!   back `String`/`[f64; N]`.
//!
//! Imports that pass only scalars (`sleep`, `signal_notify`, …) are re-exported
//! directly and called as `abi::sleep(…)` in an `unsafe` block: those calls
//! carry no addresses. Anything with a pointer in its signature is reached
//! through [`marshal`] instead.
//!
//! The one pointer outside this module is the `#[global_allocator]` in
//! [`__rt`](crate::__rt) — `GlobalAlloc`'s own trait methods are defined in terms
//! of `*mut u8`, so it has no choice. It forwards straight to
//! `marshal::realloc` (wasm-only, hence no link from a host-target doc build).
//!
//! **SDK-internal.** The module is `pub` so a plugin SDK layering its own import
//! module on top (entities, effects, …) can build on [`marshal`]'s two-call read
//! protocol instead of copying it. Animation code never sees it.

#[cfg(target_arch = "wasm32")]
#[path = "wasm.rs"]
mod sys;
#[cfg(not(target_arch = "wasm32"))]
#[path = "stubs.rs"]
mod sys;

pub mod marshal;

// The scalar-only imports, callable directly. The pointer-taking ones live here
// too — `marshal` is built on them — but nothing outside this module calls those.
pub use sys::*;

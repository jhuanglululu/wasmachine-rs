//! WASMachine's guest-side core: the half of a WASM animation that has nothing
//! to do with any particular plugin.
//!
//! Tasks (fork wrapped as [`spawn`]), [`sleep`], the [`sync`] primitives, the
//! [`random`] streams, the read-only [`env`], the [`math`] types, the panic hook
//! and the host-backed allocator ([`__rt`]) — everything a guest module needs
//! before it says a word
//! about *what it is animating*. Plugin SDKs (`billboard`, …) depend on this
//! crate, add their own import module and domain types, and re-export what is
//! here so animation authors see one API.
//!
//! Everything user-facing is safe Rust: the raw pointers and `extern` blocks
//! live in [`abi`] alone, with host-target stubs so `cargo test` runs natively.
//!
//! # The memory model
//!
//! A task's memory is its own: [`spawn`] forks, which deep-copies the parent's
//! linear memory, and nothing a task writes afterwards is visible to any other.
//! The single exception is the engine's **shared static region** — a second
//! address window the host maps for the whole instance, allocated from once and
//! never freed, which a fork references rather than copies. It is not
//! animation-facing; [`env`] is what it currently holds, which is why an
//! environment value is a `&'static str` shared by every task.
//!
//! ```ignore
//! use wasmachine::{env, sleep, spawn};
//! use wasmachine::math::Ticks;
//! use wasmachine::sync::Signal;
//!
//! let go = Signal::new();
//! let worker = spawn(move || {
//!     go.wait();
//!     sleep(Ticks::new(20));
//! });
//! go.notify_all();
//! worker.join();
//!
//! let speed = env::get("speed").unwrap_or("1.0");
//! ```

pub mod abi;
pub mod env;
pub mod math;
pub mod random;
pub mod sync;
mod task;

#[doc(hidden)]
#[path = "rt.rs"]
pub mod __rt;

pub use task::{Task, sleep, spawn};

/// The engine ABI this crate speaks. The SDK's `main` attribute exports it as
/// `_engine_abi`, so the host can refuse a mismatched module at load time
/// rather than at first use.
///
/// Version 2 is the `"engine"` import module: memory and tasks, sync, random,
/// the math kernel, and — new in 2 — the host-owned shared static region
/// (`shared_alloc`) together with the read-only environ (`environ_len` /
/// `environ_read`) that lives in it. (Version 1 was the same module without
/// those three imports; fork semantics are identical in both.) Additive import
/// growth does not bump it (an older module simply imports fewer functions);
/// semantic changes do, and a second address window is one. Plugin modules
/// version independently, through their own handshake export.
pub const ENGINE_ABI_VERSION: i32 = 2;

/// Write a debug message to the server console.
pub fn log(msg: &str) {
    abi::marshal::log(msg);
}

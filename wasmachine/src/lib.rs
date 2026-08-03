//! WASMachine's guest-side core: the half of a WASM animation that has nothing
//! to do with any particular plugin.
//!
//! Tasks ([`spawn`] and [`scope`]), [`sleep`], the [`sync`] primitives, the
//! [`random`] streams, the read-only [`environ`], the [`math`] types, the panic
//! hook and the host-backed allocator ([`__rt`]) — everything a guest module
//! needs before it says a word about *what it is animating*. Plugin SDKs
//! (`billboard`, …) depend on this crate, add their own import module and
//! domain types, and re-export what is here so animation authors see one API.
//!
//! Everything user-facing is safe Rust: the raw pointers and `extern` blocks
//! live in [`abi`] alone, with host-target stubs so `cargo test` runs natively.
//!
//! # The memory model
//!
//! Every task of one animation runs in **one shared linear memory**. Tasks are
//! cooperative and never parallel — a switch happens only at a blocking point
//! (`sleep`, `join`, a sync or channel op) — so there is no data race to guard
//! against, but there is genuine aliasing: what one task writes, the next one
//! to run sees. Moving an owning handle into a task is therefore ordinary Rust
//! (one allocation, one owner, one drop), spawning copies nothing, and
//! [`scope`] can hand a task a *borrow*. See [`spawn`] for what a task costs.
//!
//! # Build requirements
//!
//! A guest module built on this crate must export its shadow stack pointer, so
//! the host can give each new task its own stack region:
//!
//! ```text
//! # .cargo/config.toml of the animation crate
//! [target.wasm32-unknown-unknown]
//! rustflags = ["-C", "link-arg=--export=__stack_pointer"]
//! ```
//!
//! Without it the host refuses to construct the instance, with an error naming
//! the flag. Guests are also built `panic = "abort"` (a panic routes through the
//! hook to the host's `fail`), which is what lets [`scope`] and the task
//! trampoline ignore unwinding entirely.
//!
//! ```ignore
//! use wasmachine::{environ, scope, sleep, spawn};
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
//! // Borrowing tasks, all joined before `scope` returns.
//! let panels = vec![1, 2, 3];
//! scope(|s| {
//!     for panel in &panels {
//!         s.spawn(move || sleep(Ticks::new(*panel)));
//!     }
//! });
//!
//! let speed = environ().get("speed").unwrap_or("1.0");
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

pub use env::{Environ, environ};
pub use task::{Scope, ScopedTask, Task, scope, sleep, spawn};

/// The engine ABI this crate speaks. The SDK's `main` attribute exports it as
/// `_engine_abi`, so the host can refuse a mismatched module at load time
/// rather than at first use.
///
/// Version 2 is the `"engine"` import module: one shared linear memory and
/// `spawn(entry, data)` tasks, the read-only environ, sync, random, and the
/// math kernel. (Version 1 was the same module with fork-copied per-task
/// memory and a `fork()` import instead.) Additive import growth does not bump
/// it (an older module simply imports fewer functions); semantic changes do.
/// Plugin modules version independently, through their own handshake export.
pub const ENGINE_ABI_VERSION: i32 = 2;

/// Write a debug message to the server console.
pub fn log(msg: &str) {
    abi::marshal::log(msg);
}

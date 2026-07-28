//! WASMachine's guest-side core: the half of a WASM animation that has nothing
//! to do with any particular plugin.
//!
//! Tasks (fork wrapped as [`spawn`]), [`sleep`], the [`sync`] primitives, the
//! [`random`] streams, the [`math`] types, the panic hook and the host-backed
//! allocator ([`__rt`]) — everything a guest module needs before it says a word
//! about *what it is animating*. Plugin SDKs (`billboard`, …) depend on this
//! crate, add their own import module and domain types, and re-export what is
//! here so animation authors see one API.
//!
//! Everything user-facing is safe Rust: the raw pointers and `extern` blocks
//! live in [`abi`] alone, with host-target stubs so `cargo test` runs natively.
//!
//! ```ignore
//! use wasmachine::{sleep, spawn};
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
//! ```

pub mod abi;
pub mod math;
pub mod random;
pub mod sync;
mod task;

#[doc(hidden)]
#[path = "rt.rs"]
pub mod __rt;

pub use task::{Task, sleep, spawn};

/// Write a debug message to the server console.
pub fn log(msg: &str) {
    abi::marshal::log(msg);
}

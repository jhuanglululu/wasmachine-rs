//! Runtime glue: init (called by the SDK's `main` attribute) and the
//! host-backed global allocator.
//!
//! Note the absence of any synchronization: tasks are cooperative coroutines
//! that only switch at blocking points (`sleep`, `join`), and a fork copies
//! the whole memory, so no state here is ever shared or contended.

/// Called by the generated entry export before the user's `main`. Routes
/// panics to host `fail` so every guest error kills the animation with a
/// readable message instead of a bare trap.
pub fn init() {
    #[cfg(target_arch = "wasm32")]
    {
        std::panic::set_hook(Box::new(|info| {
            let msg = info.to_string();
            crate::abi::marshal::fail(&msg)
        }));
    }
}

/// Emitted by the SDK's `main` attribute for `random_seed = N`, right after
/// [`init`]:
/// reseed the host's deterministic stream and route
/// [`default_random`](crate::random::default_random) to it. Runs before the
/// user's `main` and before any task exists, so the routing flag is immutable
/// from then on and every forked task inherits the same answer.
pub fn seed_random(seed: i64) {
    crate::random::init_seeded(seed);
}

#[cfg(target_arch = "wasm32")]
mod host_alloc {
    use core::alloc::{GlobalAlloc, Layout};

    /// Forwards every allocation to the host `realloc` import; the Java side
    /// owns the actual allocator (and the per-animation memory cap).
    struct HostAlloc;

    unsafe impl GlobalAlloc for HostAlloc {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            unsafe {
                crate::abi::marshal::realloc(
                    core::ptr::null_mut(),
                    0,
                    layout.align(),
                    layout.size(),
                )
            }
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            unsafe {
                crate::abi::marshal::realloc(ptr, layout.size(), layout.align(), 0);
            }
        }

        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            unsafe { crate::abi::marshal::realloc(ptr, layout.size(), layout.align(), new_size) }
        }
    }

    #[global_allocator]
    static HOST_ALLOC: HostAlloc = HostAlloc;
}

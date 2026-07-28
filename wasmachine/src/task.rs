//! Tasks. The ABI primitive is Linux-style `fork` (duplicate the current
//! task, full linear-memory copy — process semantics, nothing shared); the
//! safe API wraps it as `spawn(closure)`.
//!
//! The closure is `Sync`-bounded, and a plugin SDK's resource owner handles
//! (entities, …) are deliberately `!Sync`: capturing one is a *compile error*.
//! That guard is what lets handles stay simple moves (no owner bookkeeping, no
//! runtime checks) — the owner provably never crosses into a child task. The
//! SDK's weak references (`Sync + Clone`) are what a child task captures
//! instead.

use crate::abi;
use crate::math::Ticks;

/// A spawned task. Dropping it detaches the task (it still ends when the
/// animation ends).
#[derive(Debug)]
pub struct Task {
    id: i32,
}

/// Run `f` in a new task, interleaved with this one at blocking points.
///
/// Under the hood this forks the whole task, Linux-style: the child gets a
/// full copy of memory, runs `f`, and exits. The parent returns immediately
/// with the child's handle. Nothing but the fork itself crosses the ABI —
/// no closures, no function pointers.
pub fn spawn(f: impl FnOnce() + Sync + 'static) -> Task {
    match unsafe { abi::fork() } {
        0 => {
            f();
            unsafe { abi::exit() }
        }
        id => Task { id },
    }
}

impl Task {
    /// Park the current task until this task ends.
    pub fn join(self) {
        unsafe { abi::join(self.id) }
    }

    /// End this task immediately, at its next scheduling opportunity.
    ///
    /// The killed task's destructors **never run**, so any resources it owned
    /// are *orphaned*: they stay alive until the animation ends (the host
    /// releases everything then), and weak references to them keep working.
    /// Kill is a tool for cutting a show short — prefer letting tasks finish
    /// so their RAII cleanup runs.
    pub fn kill(self) {
        unsafe { abi::kill(self.id) }
    }
}

/// Park the current task for `ticks` game ticks (20 ticks = 1 second).
/// Everything between two blocking points runs uninterrupted — no other task
/// can observe or interleave with it.
pub fn sleep(ticks: Ticks) {
    let t = i64::try_from(ticks.count()).expect("sleep duration overflows i64");
    unsafe { abi::sleep(t) }
}

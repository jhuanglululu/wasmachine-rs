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
///
/// # What a task costs
///
/// "Full copy of memory" is literal: the host duplicates the entire linear
/// memory, plus the stack, frames, globals and tables, at the moment of the
/// fork. So a fork is *not* cheap the way an async task is cheap — it is
/// proportional to how big the animation's memory already is, paid in one
/// blocking-point-free burst, and `N` live tasks hold `N` copies of it.
///
/// That copy includes the parent's live stack, owner handles and all — but the
/// child never unwinds it: `f` returns into the ABI's `exit`, which never
/// returns, and guests are built `panic = "abort"`. Nothing in the child's
/// inherited copy is ever dropped, so a child finishing (or dying) despawns
/// nothing the parent owns.
///
/// Against the host's per-instance memory cap, what each live task charges is
/// its own guest heap, plus what the animation has queued in channels — one
/// allowance shared by every task rather than a fresh one per task. Exceeding
/// it is not a throttle: the reservation is refused and the animation is killed
/// with a message naming the overflow. The cap's *value* is host configuration,
/// not something this crate pins, so the only portable rule is the shape: heap
/// per task multiplies.
///
/// There is no cap on the task count itself, which means the practical ceiling
/// is that multiplication — an animation with a fat heap runs out at a handful
/// of tasks, a lean one at many. For scale: an animation whose per-task heap is
/// the usual few kilobytes of frame state fits tens of tasks inside a
/// 16-MiB-class cap without the memory mattering at all, so unless the animation
/// holds something genuinely large per task, memory is not what should decide
/// the count. What should is legibility — concurrency you cannot see on screen
/// is not worth animating, and a handful of named moving parts is easier to
/// reason about (and to keep out of deadlock) than dozens.
/// Budget a task per genuinely concurrent moving
/// part, not per unit of work; a loop over frames in one task costs nothing
/// extra, and [`sync`](crate::sync) primitives are how a fixed set of tasks
/// coordinates without spawning more.
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

//! Tasks. Every task of one animation runs in **one shared linear memory**:
//! the ABI primitive is `spawn(entry, data)`, where `entry` is a function-table
//! index and `data` an opaque guest pointer. The safe API wraps it as
//! [`spawn`] (a `'static` closure, detached or joined at will) and [`scope`]
//! (borrowing closures, all joined before the scope returns).
//!
//! **One memory, one drop.** Tasks are cooperative — they never run in
//! parallel, and a switch only happens at a blocking point (`sleep`, `join`,
//! a channel or sync op) — but they genuinely alias the same heap. So *moving*
//! an owning handle (a plugin SDK's entity owner, a `Receiver`, a `Box`) into a
//! child task is ordinary, sound Rust: there is one allocation, one owner, one
//! drop. The bound here is [`Send`], not `Sync`, and nothing needs to be
//! `!Sync` to stay safe.
//!
//! Only the stack, the frames, the globals and the table are per task; the host
//! gives each new task its own stack region out of the same heap. Guests must
//! therefore export `__stack_pointer` so the host can point a child at its
//! region — see the [crate docs](crate) for the required link argument.

use core::cell::RefCell;
use core::marker::PhantomData;

use crate::abi;
use crate::math::Ticks;

/// A spawned task. Dropping it detaches the task (it still ends when the
/// animation ends).
#[derive(Debug)]
pub struct Task {
    id: i32,
}

/// Hand a boxed closure to the host as a new task, without checking that
/// anything `f` borrows outlives it.
///
/// The mechanism, which is the whole of the new ABI's guest side:
///
/// - `f` is boxed. `Box<F>` is a *thin* pointer for a concrete `F`, which is
///   what lets it cross as a single `i32` (no `dyn`, no fat pointer).
/// - [`trampoline`] is monomorphised for that same `F`, so it knows statically
///   what the pointer points at. On `wasm32` a function pointer *is* its
///   index in the module's function table, which is exactly what the host's
///   `entry` argument wants — so the cast is the conversion.
/// - The host allocates the child a stack region, sets its `__stack_pointer`,
///   and calls `entry(data)`. The trampoline reconstitutes the `Box`, runs the
///   closure, and calls `exit`.
///
/// Ownership crosses with the pointer: [`Box::into_raw`] leaks it here on
/// purpose, and the child's `Box::from_raw` takes it back. The parent must
/// **not** free it — under one shared memory that would be a double free, not
/// the harmless double-drop-of-a-copy that fork semantics used to make of it.
///
/// # Safety
///
/// Everything `f` borrows must stay alive and untouched until the task ends.
/// [`spawn`] discharges this with `'static`; [`Scope`] discharges it by joining
/// every task it started before the borrowed data can go away.
unsafe fn spawn_raw<F: FnOnce() + Send>(f: F) -> i32 {
    /// What the host actually calls, once, in the child task.
    extern "C" fn trampoline<F: FnOnce()>(data: i32) {
        // Safety: `data` is the pointer `spawn_raw` leaked for this exact `F`,
        // and the host calls this trampoline exactly once for it.
        let f = unsafe { Box::from_raw(data as usize as *mut F) };
        f();
        // Never returns, so the closure's `Box` is the only thing that was
        // ever dropped here — nothing unwinds this stack (guests are
        // `panic = "abort"`).
        unsafe { abi::exit() }
    }

    // On wasm32 both a data pointer and a function pointer are 32 bits, so
    // neither cast loses anything. (On the host target the ABI is stubbed out
    // and panics before the truncation could matter.)
    let data = Box::into_raw(Box::new(f)) as usize as i32;
    let entry = trampoline::<F> as extern "C" fn(i32) as usize as i32;
    unsafe { abi::spawn(entry, data) }
}

/// Run `f` in a new task, interleaved with this one at blocking points.
///
/// The closure is boxed and handed to the host along with a trampoline's
/// function-table index; the child runs it and exits. The parent returns
/// immediately with the child's handle. `'static` is what makes this safe
/// without any joining discipline — a detached task may outlive every local
/// in sight. To let a task *borrow*, use [`scope`] instead.
///
/// # What a task costs
///
/// Tasks share one linear memory, so spawning copies nothing: the cost is the
/// boxed closure plus the host-allocated **stack region** for the new task
/// (host configuration — 64 KiB at the time of writing), both charged to the
/// same per-instance memory allowance as the rest of the heap. The region is
/// released when the task ends. Exceeding the allowance is not a throttle: the
/// reservation is refused and the animation is killed with a message naming the
/// overflow.
///
/// So the ceiling is real but generous, and it is no longer "the whole heap,
/// times the task count" the way fork-copies made it. What should decide the
/// task count is legibility: concurrency you cannot see on screen is not worth
/// animating, and a handful of named moving parts is easier to reason about
/// (and to keep out of deadlock) than dozens. Budget a task per genuinely
/// concurrent moving part, not per unit of work; a loop over frames in one task
/// costs nothing extra, and [`sync`](crate::sync) primitives are how a fixed
/// set of tasks coordinates without spawning more.
///
/// # Borrowing is a compile error
///
/// ```compile_fail
/// let frames = vec![1, 2, 3];
/// // `frames` is borrowed, but the task may outlive it: rejected.
/// let task = wasmachine::spawn(|| println!("{}", frames.len()));
/// task.join();
/// ```
pub fn spawn(f: impl FnOnce() + Send + 'static) -> Task {
    // Safety: `'static` — the closure borrows nothing that can end.
    Task {
        id: unsafe { spawn_raw(f) },
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

/// Run `f` with a [`Scope`] whose tasks may **borrow** from the enclosing
/// function, joining every one of them before returning.
///
/// This mirrors [`std::thread::scope`]. The lifetime dance is the same, and so
/// is the payoff: `s.spawn(…)` takes `FnOnce() + Send + 'scope` rather than
/// `'static`, so a task can read a local slice or write through a `&mut` split
/// off from one, and `scope` cannot return until they are all done with it.
///
/// ```no_run
/// use wasmachine::{scope, sleep};
/// use wasmachine::math::Ticks;
///
/// let frames = vec![1.0, 2.0, 3.0];
/// scope(|s| {
///     s.spawn(|| {
///         // Borrowed, not moved: `frames` outlives the scope.
///         let _first = frames[0];
///         sleep(Ticks::new(20));
///     });
///     s.spawn(|| println!("{}", frames.len()));
/// });
/// // Both tasks have ended here, so `frames` is free to go.
/// ```
///
/// # No unwinding path
///
/// Guests are built `panic = "abort"` and a panic routes through the SDK's hook
/// to the host's `fail` (which kills the animation), so there is no unwind that
/// could skip the joins — the join-on-normal-return *is* the whole story, and
/// no drop guard is needed to make it one.
///
/// # The sharp edge: `kill` inside a scope
///
/// [`ScopedTask::kill`] is as dangerous here as [`Task::kill`] is anywhere, and
/// then some. A killed task's destructors never run, so anything it owned is
/// orphaned until the animation ends; on top of that the host reclaims the dead
/// task's **stack region**, and a scoped task's stack is exactly where its
/// borrows of the parent's data live. Killing a task that another task is
/// meanwhile reading *through* — a `&mut` split handed around, a structure the
/// killed task was mid-write on — leaves that reader looking at reclaimed
/// memory, which no lifetime in this API can catch. Prefer a
/// [`Signal`](crate::sync::Signal) the task checks and returns on; reach for
/// `kill` only for a task you know borrows nothing live.
pub fn scope<'env, F, T>(f: F) -> T
where
    F: for<'scope> FnOnce(&'scope Scope<'scope, 'env>) -> T,
{
    let scope = Scope {
        running: RefCell::new(Vec::new()),
        _scope: PhantomData,
        _env: PhantomData,
    };
    let result = f(&scope);
    // Everything `f` started and did not join itself, joined now — before the
    // caller's locals (which those tasks may borrow) can go anywhere.
    //
    // Taken out through a shared borrow rather than `into_inner()`: `&'scope
    // Scope<'scope, ..>` makes the borrow `f` took last as long as the scope
    // itself, so the scope can never be *moved out of* afterwards. Reading it
    // is fine, and no task is parked in `borrow_mut` because spawning does not
    // park.
    let ids = core::mem::take(&mut *scope.running.borrow_mut());
    for id in ids {
        unsafe { abi::join(id) }
    }
    result
}

/// The handle [`scope`] hands its closure: tasks spawned through it may borrow
/// from the scope's environment, because the scope joins them before it returns.
///
/// The two lifetimes are the ones [`std::thread::Scope`] uses, and they are
/// invariant for the same reason: `'scope` is how long the scope itself lasts
/// (so a spawned closure may not outlive it), and `'env` bounds what the
/// closure may borrow (so it must outlive the scope).
///
/// A `Scope` is deliberately not `Sync`: a scoped task cannot capture `&Scope`
/// and spawn *further* scoped tasks. Nesting a whole `scope(…)` inside a task
/// is the way to express that, and it keeps this type a plain [`RefCell`] with
/// no cross-task bookkeeping to get wrong.
#[derive(Debug)]
pub struct Scope<'scope, 'env: 'scope> {
    /// Ids of tasks started here and not yet joined. Only the task that owns
    /// the scope ever touches this — spawning does not park, so no other task
    /// can observe it mid-update.
    running: RefCell<Vec<i32>>,
    _scope: PhantomData<&'scope mut &'scope ()>,
    _env: PhantomData<&'env mut &'env ()>,
}

impl<'scope, 'env> Scope<'scope, 'env> {
    /// Run `f` in a new task which the enclosing [`scope`] will join before it
    /// returns. Unlike [`spawn`], `f` may borrow anything that outlives `'env`.
    pub fn spawn<F>(&'scope self, f: F) -> ScopedTask<'scope, 'env>
    where
        F: FnOnce() + Send + 'scope,
    {
        // Safety: `f` borrows only for `'env`, which outlives `'scope`, and
        // `scope` joins this task (or `ScopedTask::join` does) before `'scope`
        // ends — so nothing `f` borrows can die while the task is live.
        let id = unsafe { spawn_raw(f) };
        self.running.borrow_mut().push(id);
        ScopedTask { id, scope: self }
    }

    /// Forget a task: it is no longer this scope's to join.
    fn release(&self, id: i32) {
        self.running.borrow_mut().retain(|&other| other != id);
    }
}

/// A task started by [`Scope::spawn`]. Holding one is optional — the scope
/// joins whatever is left when it ends — but it lets a task be joined (or
/// killed) *early*, which is the only way to observe its end before the scope's
/// own.
#[derive(Debug)]
pub struct ScopedTask<'scope, 'env: 'scope> {
    id: i32,
    scope: &'scope Scope<'scope, 'env>,
}

impl ScopedTask<'_, '_> {
    /// Park the current task until this one ends. The scope will not join it
    /// again.
    pub fn join(self) {
        self.scope.release(self.id);
        unsafe { abi::join(self.id) }
    }

    /// End this task immediately, at its next scheduling opportunity. The
    /// scope will not join it.
    ///
    /// Read [`scope`]'s "sharp edge" section before using this: a scoped task's
    /// stack region holds its borrows of the parent's data, and killing it
    /// reclaims that region without running a single destructor.
    pub fn kill(self) {
        self.scope.release(self.id);
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

//! Cross-task synchronization: [`Signal`], [`Barrier`], the [`Waitable`]
//! combinators, and bounded MPSC channels ([`channel`], [`Sender`],
//! [`Receiver`]).
//!
//! A fork copies the whole linear memory, so tasks share **no Rust data** —
//! every one of these primitives is a *host-side* object addressed by an
//! `i32` id. That id is a plain integer sitting in the copied memory, so every
//! handle here is `Sync` and survives fork for free: create the primitive before
//! [`spawn`](crate::spawn), capture the handle in the closure, and both sides
//! talk to the same host object.
//!
//! [`Signal`], [`Barrier`] and [`Composite`] are additionally `Clone + Copy` —
//! hand them to as many tasks as you like. A channel splits that: [`Sender`] is
//! `Clone`, one per producer, while [`Receiver`] is deliberately **move-only**,
//! which is what makes the channel single-consumer at compile time instead of by
//! convention.
//!
//! ```ignore
//! let ready = Barrier::new(3);
//! let go = Signal::new();
//! for i in 0..3 {
//!     spawn(move || {
//!         ready.wait();          // all three arrive together
//!         go.wait();             // then wait for the conductor
//!         run_panel(i);
//!     });
//! }
//! go.notify_all();
//! ```
//!
//! Handles have no `Drop`: host sync objects live until the animation ends
//! (their count is capped per instance, and exceeding the cap kills loudly).
//! Nothing here needs atomics — scheduling is cooperative and a task runs
//! uninterrupted between two blocking points.

mod channel;

pub use channel::{Receiver, Sender, channel};

use crate::abi;

/// Which parked waiter [`Signal::notify_one`] releases.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Policy {
    /// The task that has been parked the longest (FIFO).
    Oldest,
    /// The task that parked most recently (LIFO).
    Newest,
    /// A uniformly random parked task. The draw comes from a host-side
    /// *scheduling* RNG, separate from guest-facing randomness — cosmetic
    /// `random()` calls can never reshuffle scheduling.
    Random,
}

impl Policy {
    /// Wire value for the `signal_notify` import (`0` is notify-all).
    const fn wire(self) -> i32 {
        match self {
            Policy::Oldest => 1,
            Policy::Newest => 2,
            Policy::Random => 3,
        }
    }
}

mod sealed {
    /// The id every waitable is, at the ABI. Sealed: `Waitable` is a closed
    /// set of host object kinds, not an extension point.
    pub trait HasWaitableId {
        fn waitable_id(&self) -> i32;
    }
}

use sealed::HasWaitableId;

/// Anything a task can park on: [`Signal`], [`Barrier`], and the composites
/// built by [`and`](Waitable::and) / [`or`](Waitable::or).
///
/// Composites are host objects too, created eagerly when you combine — so
/// `a.or(&b).and(&c)` builds a boolean tree host-side and parks on its root.
/// They **latch per waiter**: a leaf that fires while you are parked is
/// remembered, and you are released once your tree completes.
pub trait Waitable: HasWaitableId {
    /// Park the current task until this waitable fires.
    ///
    /// Parking on a tree containing a [`Barrier`] counts as an arrival at that
    /// barrier; if another arm of an `or` wins the race instead, the arrival
    /// is taken back.
    fn wait(&self) {
        unsafe { abi::wait(self.waitable_id()) }
    }

    /// Fires when **both** fire (in any order, across any number of ticks).
    fn and(&self, other: &impl Waitable) -> Composite {
        Composite {
            id: unsafe { abi::wait_all(self.waitable_id(), other.waitable_id()) },
        }
    }

    /// Fires when **either** fires.
    fn or(&self, other: &impl Waitable) -> Composite {
        Composite {
            id: unsafe { abi::wait_any(self.waitable_id(), other.waitable_id()) },
        }
    }
}

impl<T: HasWaitableId> Waitable for T {}

/// A one-to-many wakeup. Parked tasks are released by [`notify_all`] or one at
/// a time by [`notify_one`]; a notify with nobody parked is simply lost (a
/// signal is an event, not a counter).
///
/// [`notify_all`]: Signal::notify_all
/// [`notify_one`]: Signal::notify_one
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Signal {
    id: i32,
}

impl Signal {
    pub fn new() -> Signal {
        Signal {
            id: unsafe { abi::signal_new() },
        }
    }

    /// Release every parked task. They resume in spawn order, same tick.
    pub fn notify_all(&self) {
        unsafe { abi::signal_notify(self.id, 0) }
    }

    /// Release exactly one parked task, chosen by `policy`.
    pub fn notify_one(&self, policy: Policy) {
        unsafe { abi::signal_notify(self.id, policy.wire()) }
    }
}

impl Default for Signal {
    fn default() -> Signal {
        Signal::new()
    }
}

impl HasWaitableId for Signal {
    fn waitable_id(&self) -> i32 {
        self.id
    }
}

/// A rendezvous for `n` tasks: the first `n - 1` to [`wait`](Waitable::wait)
/// park, and the `n`-th releases them all together (same tick, spawn order).
///
/// The barrier then rearms, so the same handle choreographs a repeating
/// lock-step animation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Barrier {
    id: i32,
}

impl Barrier {
    /// A barrier that releases once `n` tasks have arrived. `n` must be at
    /// least 1 and fit in the ABI's `i32`; anything else is a bug and kills
    /// the animation.
    pub fn new(n: u32) -> Barrier {
        assert!(n >= 1, "Barrier::new requires at least one participant");
        let n = i32::try_from(n).expect("Barrier participant count overflows i32");
        Barrier {
            id: unsafe { abi::barrier_new(n) },
        }
    }
}

impl HasWaitableId for Barrier {
    fn waitable_id(&self) -> i32 {
        self.id
    }
}

/// An `and`/`or` combination of waitables, itself waitable — so combinators
/// chain: `barrier.or(&sig1).and(&sig2)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Composite {
    id: i32,
}

impl HasWaitableId for Composite {
    fn waitable_id(&self) -> i32 {
        self.id
    }
}

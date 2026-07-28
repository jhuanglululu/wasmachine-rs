//! Bounded MPSC channels over host-side buffers.
//!
//! Why a `(Sender, Receiver)` split rather than one `Channel<T>` handle: the
//! host queue is *multi-producer, single-consumer*, and the split is what makes
//! that shape true instead of merely documented. [`Sender`] is `Clone + Sync`
//! (hand out one per producer task); [`Receiver`] is `Sync` but deliberately
//! **not** `Clone`, so it moves — exactly once — into whichever task drains
//! the channel, the same move-only discipline entity owner handles use. Two
//! tasks racing on `recv` would be a real bug (each element goes to exactly one
//! of them), and here it simply cannot be written.
//!
//! Payloads cross as the raw bytes of `T`, which is why `T: Pod`: a `String`,
//! `Vec`, or reference in a payload is a compile error, because the receiving
//! task's memory is a *copy* — the heap those types point into does not exist
//! over there. Padding is rejected at compile time too (uninitialized padding
//! bytes have no defined value to copy). Derive `Pod + Zeroable` (a plugin SDK
//! puts both in its prelude) on your own `#[repr(C)]` structs; the math types
//! here already do.

use core::marker::PhantomData;

use bytemuck::Pod;

use crate::abi;
use crate::abi::marshal;

/// Create a bounded channel with room for `capacity` queued elements.
///
/// `send` parks when the queue is full, `recv`/`peek` park when it is empty;
/// admission when a full channel drains is FIFO by park order.
///
/// ```ignore
/// let (tx, rx) = channel::<Position>(8);
/// for i in 0..3 {
///     let tx = tx.clone();
///     spawn(move || tx.send(Position::new(i as f64, 0.0, 0.0)));
/// }
/// let first = rx.recv();
/// ```
pub fn channel<T: Pod>(capacity: usize) -> (Sender<T>, Receiver<T>) {
    let cap = i32::try_from(capacity).expect("channel capacity overflows i32");
    assert!(cap > 0, "channel capacity must be at least 1");
    let id = unsafe { abi::channel_new(cap) };
    (
        Sender {
            id,
            _payload: PhantomData,
        },
        Receiver {
            id,
            _payload: PhantomData,
        },
    )
}

/// The producing half. `Clone + Sync`: clone one per producer task before
/// spawning them.
#[derive(Debug)]
pub struct Sender<T> {
    id: i32,
    // fn() -> T: the handle is Send + Sync whatever T is (it holds no T, just
    // the host id), same trick the entity weak references use.
    _payload: PhantomData<fn() -> T>,
}

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Sender<T> {
        Sender {
            id: self.id,
            _payload: PhantomData,
        }
    }
}

impl<T: Pod> Sender<T> {
    /// Queue a value, parking the current task while the channel is full.
    pub fn send(&self, value: T) {
        marshal::channel_send(self.id, bytemuck::bytes_of(&value));
    }
}

/// The consuming half. `Sync` so it can be captured by a spawned closure, but
/// **not** `Clone`: moving it is what makes the channel single-consumer.
#[derive(Debug)]
pub struct Receiver<T> {
    id: i32,
    _payload: PhantomData<fn() -> T>,
}

impl<T: Pod> Receiver<T> {
    /// Take the front element, parking the current task while empty.
    pub fn recv(&self) -> T {
        let len = unsafe { abi::channel_recv_len(self.id) };
        self.read(len, Take::Pop)
    }

    /// Read the front element without removing it, parking while empty.
    pub fn peek(&self) -> T {
        let len = unsafe { abi::channel_peek_len(self.id) };
        self.read(len, Take::Leave)
    }

    /// Take the front element if one is queued right now; never parks.
    pub fn try_recv(&self) -> Option<T> {
        let len = unsafe { abi::channel_try_len(self.id) };
        (len >= 0).then(|| self.read(len, Take::Pop))
    }

    /// Read the front element if one is queued right now; never parks.
    pub fn try_peek(&self) -> Option<T> {
        let len = unsafe { abi::channel_try_len(self.id) };
        (len >= 0).then(|| self.read(len, Take::Leave))
    }

    /// Drop every queued element. Tasks parked in `send` are admitted as room
    /// appears, so a `clear` can release producers.
    pub fn clear(&self) {
        unsafe { abi::channel_clear(self.id) }
    }

    /// Copy the front element out. `len` is what the host just reported for
    /// it, and there is no blocking point between that report and the copy,
    /// so the front cannot have changed underneath us.
    fn read(&self, len: i32, take: Take) -> T {
        let len = usize::try_from(len).expect("host reported a negative channel payload length");
        let mut value = T::zeroed();
        let buf = bytemuck::bytes_of_mut(&mut value);
        assert!(
            len == buf.len(),
            "channel payload is {} bytes but this channel's type is {} bytes \
             — a sender and a receiver disagree on the payload type",
            len,
            buf.len(),
        );
        match take {
            Take::Pop => marshal::channel_recv(self.id, buf),
            Take::Leave => marshal::channel_peek(self.id, buf),
        }
        value
    }
}

/// Whether a read pops the element or leaves it queued.
#[derive(Clone, Copy)]
enum Take {
    Pop,
    Leave,
}

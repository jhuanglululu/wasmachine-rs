//! Randomness, in three tiers behind one [`Rng`] trait.
//!
//! | Tier | Type | Use it when |
//! |---|---|---|
//! | pure guest | [`SplitRng`] | you want a stream you control, reproduce, and hand to other tasks |
//! | host, non-deterministic | [`HostRng`] | a different show every run (the default) |
//! | host, deterministic | [`HostSeededRng`] | the same show for the same viewer every visit |
//!
//! [`default_random`] picks between the two host streams for you:
//! non-deterministic normally, the deterministic stream when the animation
//! declared `random_seed = N` on the SDK's `main` attribute.
//!
//! **Why the deterministic default lives host-side.** A guest-global RNG would
//! be *copied* by every fork, so two tasks seeded from it would replay the same
//! sequence — a subtly identical animation in both. The host stream is one
//! object outside the copied memory, so tasks draw from it in turn. If you do
//! want fork-copied determinism, [`SplitRng`] gives it to you explicitly:
//! `split()` before you `spawn`, and each task gets its own stream.
//!
//! ```ignore
//! let mut rng = default_random();
//! let hue = rng.range(0.0..360.0);
//! let block = *rng.choose(&[blocks::RED_CONCRETE, blocks::BLUE_CONCRETE]);
//! if rng.chance(0.25) { sparkle(); }
//!
//! let mut master = SplitRng::new(42);
//! let mut child = master.split();          // independent, before spawning
//! spawn(move || flicker(&mut child));
//! ```

use core::ops::{Range, RangeInclusive};
use core::sync::atomic::{AtomicBool, Ordering};

use bytemuck::{Pod, Zeroable};

use crate::abi;

/// A source of randomness. SDK APIs that need randomness take
/// `&mut impl Rng`, so any tier plugs in.
///
/// Implementors provide [`next_u64`](Rng::next_u64); everything else is
/// derived from it.
pub trait Rng {
    /// The next raw 64-bit draw. Every other method is built on this one.
    fn next_u64(&mut self) -> u64;

    /// A uniform `f64` in `[0, 1)`, using the top 53 bits — the full precision
    /// an `f64` has and not one bit more, so values are evenly spaced.
    fn next_f64(&mut self) -> f64 {
        // Dividing by 2^53 is exact (a power of two), so the mapping from the
        // top 53 bits to the unit interval loses nothing.
        (self.next_u64() >> 11) as f64 / 9_007_199_254_740_992.0
    }

    /// A uniform draw from a range: `rng.range(0..16)`, `rng.range(1..=6)`,
    /// `rng.range(-1.0..1.0)`. An empty range has nothing to draw and kills
    /// the animation.
    fn range<T, R: SampleRange<T>>(&mut self, range: R) -> T
    where
        Self: Sized,
    {
        range.sample(self)
    }

    /// `true` with probability `p` (`p <= 0` never, `p >= 1` always).
    fn chance(&mut self, p: f64) -> bool {
        self.next_f64() < p
    }

    /// A uniformly chosen element. An empty slice has no element to choose and
    /// kills the animation — an "empty" case here is a bug in the caller, not
    /// something to paper over with `None`.
    fn choose<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        assert!(!items.is_empty(), "Rng::choose called on an empty slice");
        &items[bounded(self, items.len() as u64) as usize]
    }
}

impl<R: Rng + ?Sized> Rng for &mut R {
    fn next_u64(&mut self) -> u64 {
        (**self).next_u64()
    }
}

/// A uniform integer in `0..span`, without modulo bias: reject the first
/// `2^64 mod span` values, after which `x % span` is exactly uniform.
fn bounded<R: Rng + ?Sized>(rng: &mut R, span: u64) -> u64 {
    debug_assert!(span > 0);
    // span.wrapping_neg() == 2^64 - span, so this is 2^64 mod span.
    let threshold = span.wrapping_neg() % span;
    loop {
        let x = rng.next_u64();
        if x >= threshold {
            return x % span;
        }
    }
}

/// The ranges [`Rng::range`] accepts. Sealed by construction: the impls below
/// are the whole set.
pub trait SampleRange<T> {
    #[doc(hidden)]
    fn sample<R: Rng + ?Sized>(self, rng: &mut R) -> T;
}

macro_rules! int_range {
    ($int:ty, $uint:ty) => {
        // The `as $uint` casts are load-bearing, including where `$int` and
        // `$uint` are the same type (`u64`/`usize`): the span *must* be computed
        // in unsigned arithmetic of the operands' own width. Widening a signed
        // subtraction instead sign-extends, which is how an `i32` range once
        // produced draws outside itself — `2_000_000_000i32.wrapping_sub(
        // -2_000_000_000)` wraps to `-294_967_296`, and `as u64` turns that into
        // a span of ~1.8e19.
        #[allow(clippy::unnecessary_cast)]
        impl SampleRange<$int> for Range<$int> {
            fn sample<R: Rng + ?Sized>(self, rng: &mut R) -> $int {
                assert!(
                    self.start < self.end,
                    "Rng::range called with an empty range"
                );
                let span = (self.end as $uint).wrapping_sub(self.start as $uint) as u64;
                self.start.wrapping_add(bounded(rng, span) as $int)
            }
        }

        #[allow(clippy::unnecessary_cast)]
        impl SampleRange<$int> for RangeInclusive<$int> {
            fn sample<R: Rng + ?Sized>(self, rng: &mut R) -> $int {
                let (start, end) = (*self.start(), *self.end());
                assert!(start <= end, "Rng::range called with an empty range");
                let width = (end as $uint).wrapping_sub(start as $uint) as u64;
                let span = width.wrapping_add(1);
                if span == 0 {
                    // A whole 64-bit type: every draw is in range already.
                    return rng.next_u64() as $int;
                }
                start.wrapping_add(bounded(rng, span) as $int)
            }
        }
    };
}

int_range!(i64, u64);
int_range!(u64, u64);
int_range!(i32, u32);
int_range!(u32, u32);
int_range!(usize, usize);

impl SampleRange<f64> for Range<f64> {
    fn sample<R: Rng + ?Sized>(self, rng: &mut R) -> f64 {
        assert!(
            self.start < self.end,
            "Rng::range called with an empty range"
        );
        self.start + (self.end - self.start) * rng.next_f64()
    }
}

/// The golden-ratio odd increment SplitMix64 advances its state by.
const GOLDEN_GAMMA: u64 = 0x9E37_79B9_7F4A_7C15;

/// A splittable pseudo-random stream: **SplitMix64**, the reference algorithm
/// (Steele/Lea/Flood, as in Java's `SplittableRandom`).
///
/// State is a 64-bit counter plus a per-stream odd increment ("gamma"):
/// `state += gamma`, then a fixed bit-mixing function of the state is the
/// output. That makes it plain data — `Pod`, so it can be *sent through a
/// [`channel`](crate::sync::channel)* — and it makes [`split`](SplitRng::split)
/// cheap: draw a fresh seed and a fresh gamma from the parent, and the child
/// walks a different orbit of the same mixer.
///
/// It is not cryptographic and not the strongest generator available; what it
/// guarantees is what animations need — bit-exact reproducibility from a seed,
/// and streams that stay visibly independent in practice.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Pod, Zeroable)]
pub struct SplitRng {
    state: u64,
    gamma: u64,
}

impl SplitRng {
    /// A stream seeded with `seed`. The same seed always produces the same
    /// sequence, on every machine and every server version.
    pub const fn new(seed: u64) -> SplitRng {
        SplitRng {
            state: seed,
            gamma: GOLDEN_GAMMA,
        }
    }

    /// A new stream, independent of this one in practice, drawn from it: this
    /// stream advances twice (once for the child's seed, once for its gamma).
    ///
    /// Split *before* [`spawn`](crate::spawn) — a fork copies memory, so a
    /// `SplitRng` captured without splitting would replay the parent's
    /// sequence in the child.
    pub fn split(&mut self) -> SplitRng {
        let state = self.next_u64();
        let gamma = mix_gamma(self.next_state());
        SplitRng { state, gamma }
    }

    /// Advance the counter and return the raw state (pre-mixing).
    fn next_state(&mut self) -> u64 {
        self.state = self.state.wrapping_add(self.gamma);
        self.state
    }
}

impl Rng for SplitRng {
    fn next_u64(&mut self) -> u64 {
        mix64(self.next_state())
    }
}

/// SplitMix64's output mixer (`mix64` / MurmurHash3-style finalizer with the
/// SplitMix constants).
const fn mix64(z: u64) -> u64 {
    let z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    let z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Turn a raw draw into a gamma for a split-off stream: MurmurHash3's
/// finalizer, forced odd, and rejected if its bit pattern has too few
/// transitions (a gamma like `0x5555…` makes a poor increment) — the same
/// filter Java's `SplittableRandom.mixGamma` applies.
const fn mix_gamma(z: u64) -> u64 {
    let z = (z ^ (z >> 33)).wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    let z = (z ^ (z >> 33)).wrapping_mul(0xC4CE_B9FE_1A85_EC53);
    let z = (z ^ (z >> 33)) | 1;
    if (z ^ (z >> 1)).count_ones() < 24 {
        z ^ 0xAAAA_AAAA_AAAA_AAAA
    } else {
        z
    }
}

/// The host's non-deterministic stream: a different sequence every run of the
/// animation. Zero-sized — the state lives host-side, so copies and forks all
/// draw from the one stream.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct HostRng;

impl Rng for HostRng {
    fn next_u64(&mut self) -> u64 {
        unsafe { abi::random_nondet() as u64 }
    }
}

/// The host's deterministic per-instance stream.
///
/// Its seed defaults to a stable host-chosen value, stable per instance
/// identity, so the same viewer sees the same variation on every visit;
/// `random_seed = N` on the SDK's `main` attribute replaces that seed with `N`
/// and makes [`default_random`] route here.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct HostSeededRng;

impl Rng for HostSeededRng {
    fn next_u64(&mut self) -> u64 {
        unsafe { abi::random_det() as u64 }
    }
}

/// Whichever host stream [`default_random`] routes to. The variant tells you
/// which one you got, so `default_random()` stays honest about it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DefaultRng {
    /// No `random_seed` was declared: a different show every run.
    NonDeterministic(HostRng),
    /// `random_seed = N` was declared.
    Deterministic(HostSeededRng),
}

impl Rng for DefaultRng {
    fn next_u64(&mut self) -> u64 {
        match self {
            DefaultRng::NonDeterministic(r) => r.next_u64(),
            DefaultRng::Deterministic(r) => r.next_u64(),
        }
    }
}

/// Set once by macro-generated init, before `main`, and never again. A fork
/// copies it, which is exactly right: it is a compile-time property of the
/// animation, identical in every task.
///
/// `AtomicBool` is not synchronization here (tasks are cooperative and share
/// no memory) — it is simply the safe way to spell a mutable global, and
/// `Relaxed` compiles to a plain load/store.
static SEEDED: AtomicBool = AtomicBool::new(false);

/// Reseed the host's deterministic stream and route [`default_random`] to it.
/// Called by the SDK's `main` attribute for `random_seed = N`; not for
/// animations to call.
#[doc(hidden)]
pub fn init_seeded(seed: i64) {
    SEEDED.store(true, Ordering::Relaxed);
    unsafe { abi::seed_random(seed) }
}

/// The randomness to reach for when you have no reason to be picky.
///
/// Routes to the host's non-deterministic stream, or to the deterministic
/// per-instance stream if the animation declared `random_seed = N` on the
/// SDK's `main` attribute.
pub fn default_random() -> DefaultRng {
    if SEEDED.load(Ordering::Relaxed) {
        DefaultRng::Deterministic(HostSeededRng)
    } else {
        DefaultRng::NonDeterministic(HostRng)
    }
}

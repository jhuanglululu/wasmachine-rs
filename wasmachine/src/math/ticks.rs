//! [`Ticks`]: a duration in game ticks (20 ticks = 1 second).

use core::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Rem, RemAssign, Sub, SubAssign};

/// A duration in game ticks; 20 ticks = 1 second.
///
/// The count is private: build one with [`Ticks::new`] / `Ticks::from(u64)`
/// and read it back with [`Ticks::count`]. Arithmetic overflow/underflow
/// panics (which kills the animation) rather than wrapping or clamping — a
/// duration that under/overflows is a bug, not something to hide.
///
/// `repr(transparent)` + `Pod`: a duration is one `u64`, so it can cross a
/// [`channel`](crate::sync::channel) on its own or inside a payload struct.
#[repr(transparent)]
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    bytemuck::Pod,
    bytemuck::Zeroable,
)]
pub struct Ticks(u64);

impl Ticks {
    /// One second's worth of ticks.
    pub const ONE_SECOND: Ticks = Ticks(20);

    /// A duration of `count` ticks.
    pub const fn new(count: u64) -> Ticks {
        Ticks(count)
    }

    /// The number of ticks in this duration.
    pub const fn count(self) -> u64 {
        self.0
    }

    /// Kills the animation if `secs` is negative — a negative duration is a
    /// bug, not something to clamp quietly.
    pub fn from_secs(secs: f64) -> Ticks {
        assert!(
            secs >= 0.0,
            "Ticks::from_secs called with a negative duration"
        );
        Ticks((secs * 20.0).round() as u64)
    }

    pub fn as_secs(self) -> f64 {
        self.0 as f64 / 20.0
    }
}

impl From<u64> for Ticks {
    fn from(count: u64) -> Ticks {
        Ticks(count)
    }
}

impl Add for Ticks {
    type Output = Ticks;
    fn add(self, rhs: Ticks) -> Ticks {
        Ticks(self.0 + rhs.0)
    }
}

impl Sub for Ticks {
    type Output = Ticks;
    fn sub(self, rhs: Ticks) -> Ticks {
        Ticks(self.0 - rhs.0)
    }
}

impl Mul<u64> for Ticks {
    type Output = Ticks;
    fn mul(self, rhs: u64) -> Ticks {
        Ticks(self.0 * rhs)
    }
}

impl Div<u64> for Ticks {
    type Output = Ticks;
    fn div(self, rhs: u64) -> Ticks {
        Ticks(self.0 / rhs)
    }
}

impl Rem<u64> for Ticks {
    type Output = Ticks;
    fn rem(self, rhs: u64) -> Ticks {
        Ticks(self.0 % rhs)
    }
}

/// How many whole `rhs`-durations fit in `self`.
impl Div<Ticks> for Ticks {
    type Output = u64;
    fn div(self, rhs: Ticks) -> u64 {
        self.0 / rhs.0
    }
}

/// The leftover after removing whole `rhs`-durations from `self`.
impl Rem<Ticks> for Ticks {
    type Output = Ticks;
    fn rem(self, rhs: Ticks) -> Ticks {
        Ticks(self.0 % rhs.0)
    }
}

impl AddAssign for Ticks {
    fn add_assign(&mut self, rhs: Ticks) {
        *self = *self + rhs;
    }
}

impl SubAssign for Ticks {
    fn sub_assign(&mut self, rhs: Ticks) {
        *self = *self - rhs;
    }
}

impl MulAssign<u64> for Ticks {
    fn mul_assign(&mut self, rhs: u64) {
        *self = *self * rhs;
    }
}

impl DivAssign<u64> for Ticks {
    fn div_assign(&mut self, rhs: u64) {
        *self = *self / rhs;
    }
}

impl RemAssign<u64> for Ticks {
    fn rem_assign(&mut self, rhs: u64) {
        *self = *self % rhs;
    }
}

impl RemAssign<Ticks> for Ticks {
    fn rem_assign(&mut self, rhs: Ticks) {
        *self = *self % rhs;
    }
}

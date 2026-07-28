//! Angles: [`Degrees`] and [`Radians`], with explicit `From` conversions in
//! both directions and the usual scalar algebra.

use core::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

/// An angle in degrees. Converts explicitly to/from [`Radians`].
///
/// The value is private: build one with [`Degrees::new`] / `Degrees::from(f64)`
/// and read it with [`Degrees::value`]. `repr(transparent)` + `Pod`, so an
/// angle can cross a [`channel`](crate::sync::channel).
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Degrees(f64);

/// An angle in radians. Converts explicitly to/from [`Degrees`].
///
/// The value is private: build one with [`Radians::new`] / `Radians::from(f64)`
/// and read it with [`Radians::value`]. `repr(transparent)` + `Pod`.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Radians(f64);

impl Degrees {
    pub const fn new(value: f64) -> Degrees {
        Degrees(value)
    }

    pub const fn value(self) -> f64 {
        self.0
    }
}

impl Radians {
    pub const fn new(value: f64) -> Radians {
        Radians(value)
    }

    pub const fn value(self) -> f64 {
        self.0
    }
}

impl From<f64> for Degrees {
    fn from(value: f64) -> Degrees {
        Degrees(value)
    }
}

impl From<f64> for Radians {
    fn from(value: f64) -> Radians {
        Radians(value)
    }
}

impl From<Degrees> for Radians {
    fn from(d: Degrees) -> Radians {
        Radians(d.0.to_radians())
    }
}

impl From<Radians> for Degrees {
    fn from(r: Radians) -> Degrees {
        Degrees(r.0.to_degrees())
    }
}

// The scalar algebra is identical for both units; a small macro keeps it
// honest and free of copy-paste drift.
macro_rules! angle_ops {
    ($t:ident) => {
        impl Add for $t {
            type Output = $t;
            fn add(self, rhs: $t) -> $t {
                $t(self.0 + rhs.0)
            }
        }
        impl Sub for $t {
            type Output = $t;
            fn sub(self, rhs: $t) -> $t {
                $t(self.0 - rhs.0)
            }
        }
        impl Neg for $t {
            type Output = $t;
            fn neg(self) -> $t {
                $t(-self.0)
            }
        }
        impl Mul<f64> for $t {
            type Output = $t;
            fn mul(self, rhs: f64) -> $t {
                $t(self.0 * rhs)
            }
        }
        impl Div<f64> for $t {
            type Output = $t;
            fn div(self, rhs: f64) -> $t {
                $t(self.0 / rhs)
            }
        }
        impl AddAssign for $t {
            fn add_assign(&mut self, rhs: $t) {
                *self = *self + rhs;
            }
        }
        impl SubAssign for $t {
            fn sub_assign(&mut self, rhs: $t) {
                *self = *self - rhs;
            }
        }
        impl MulAssign<f64> for $t {
            fn mul_assign(&mut self, rhs: f64) {
                *self = *self * rhs;
            }
        }
        impl DivAssign<f64> for $t {
            fn div_assign(&mut self, rhs: f64) {
                *self = *self / rhs;
            }
        }
    };
}

angle_ops!(Degrees);
angle_ops!(Radians);

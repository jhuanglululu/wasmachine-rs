//! [`Rotation`]: an orientation stored as a unit quaternion.

use super::angle::Radians;
use super::kernel;
use super::vectors::{Offset, Vector3d};

/// An orientation, stored as a quaternion. Build one with
/// [`Rotation::axis_angle`] or [`Rotation::euler`]; compose with `*`
/// (right-hand side applies first).
///
/// `repr(C)` + `Pod`, like the vector family: four `f64`s with no padding, so
/// a rotation can cross a [`channel`](crate::sync::channel).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Rotation {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub w: f64,
}

impl Rotation {
    pub const IDENTITY: Rotation = Rotation {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        w: 1.0,
    };

    /// Rotation of `angle` around `axis`. The axis needs a nonzero length —
    /// a zero axis is a bug and kills the animation.
    pub fn axis_angle(axis: Vector3d, angle: impl Into<Radians>) -> Rotation {
        let len = (axis.x * axis.x + axis.y * axis.y + axis.z * axis.z).sqrt();
        assert!(len > 0.0, "Rotation::axis_angle requires a nonzero axis");
        let a = angle.into().value();
        // Through the kernel, not `f64::sin_cos`: on wasm that method is a
        // software routine compiled into the module. The host has no fused
        // sincos either, so this is deliberately two crossings.
        let half = a / 2.0;
        let (s, c) = (kernel::sin(half), kernel::cos(half));
        Rotation {
            x: axis.x / len * s,
            y: axis.y / len * s,
            z: axis.z / len * s,
            w: c,
        }
    }

    /// Apply this rotation to a vector: `q · v · q⁻¹`, in the branch-free form
    /// `v + 2·(q_xyz × (q_xyz × v + w·v))`.
    ///
    /// This is what composes an SDK group's transform onto its members' local
    /// offsets, and how you point something along a direction you rotated.
    pub fn rotate(self, v: Vector3d) -> Vector3d {
        let q = Vector3d::new(self.x, self.y, self.z);
        let t = q.cross(v) + v * self.w;
        v + q.cross(t) * 2.0
    }

    /// Apply this rotation to a displacement — the same maths as
    /// [`rotate`](Rotation::rotate), keeping the [`Offset`] type.
    pub fn rotate_offset(self, v: Offset) -> Offset {
        Offset::from(self.rotate(Vector3d::from(v)))
    }

    /// The inverse rotation (the conjugate — these are unit quaternions).
    pub fn inverse(self) -> Rotation {
        Rotation {
            x: -self.x,
            y: -self.y,
            z: -self.z,
            w: self.w,
        }
    }

    /// The dot product of the two quaternions as 4-vectors. Its sign says
    /// whether they are on the same hemisphere, which is what interpolation
    /// needs to take the short way round.
    pub fn dot(self, r: Rotation) -> f64 {
        self.x * r.x + self.y * r.y + self.z * r.z + self.w * r.w
    }

    /// Normalize to a unit quaternion. A zero quaternion has no orientation, so
    /// it kills the animation rather than returning NaN.
    pub fn normalize(self) -> Rotation {
        let len = self.dot(self).sqrt();
        assert!(len > 0.0, "cannot normalize a zero quaternion");
        Rotation {
            x: self.x / len,
            y: self.y / len,
            z: self.z / len,
            w: self.w / len,
        }
    }

    /// Interpolate towards `other`, `t` clamped to `0..=1`: componentwise lerp
    /// then renormalize (*nlerp*), taking the shorter arc.
    ///
    /// Not `slerp`: nlerp follows the same path and differs only in how angular
    /// speed is distributed along it. For animation sub-steps — always small —
    /// the difference is invisible, and nlerp needs no trigonometry.
    ///
    /// Use [`lerp_unclamped`](Rotation::lerp_unclamped) when `t` may legitimately
    /// leave `0..=1`, as it does under an overshooting ease.
    pub fn lerp(self, other: Rotation, t: f64) -> Rotation {
        assert!(t.is_finite(), "Rotation::lerp called with a non-finite t");
        self.lerp_unclamped(other, t.clamp(0.0, 1.0))
    }

    /// [`lerp`](Rotation::lerp) without the clamp: a `t` outside `0..=1`
    /// extrapolates, continuing along the same arc past either end.
    ///
    /// This is what an overshooting ease needs. `Ease::BackOut` and the elastic
    /// curves deliberately leave `0..=1`, and a rotation that clamped while the
    /// position kept flying would arrive already parked at its target — the
    /// spring visible in the movement and missing from the turn.
    pub fn lerp_unclamped(self, other: Rotation, t: f64) -> Rotation {
        assert!(
            t.is_finite(),
            "Rotation::lerp_unclamped called with a non-finite t"
        );
        // Flip one end if they are on opposite hemispheres, so the blend goes
        // the short way around instead of the long way.
        let other = if self.dot(other) < 0.0 {
            Rotation {
                x: -other.x,
                y: -other.y,
                z: -other.z,
                w: -other.w,
            }
        } else {
            other
        };
        Rotation {
            x: self.x + (other.x - self.x) * t,
            y: self.y + (other.y - self.y) * t,
            z: self.z + (other.z - self.z) * t,
            w: self.w + (other.w - self.w) * t,
        }
        .normalize()
    }

    /// Yaw (around +Y), then pitch (around +X), then roll (around +Z).
    pub fn euler(
        yaw: impl Into<Radians>,
        pitch: impl Into<Radians>,
        roll: impl Into<Radians>,
    ) -> Rotation {
        Rotation::axis_angle(Vector3d::Y, yaw)
            * Rotation::axis_angle(Vector3d::X, pitch)
            * Rotation::axis_angle(Vector3d::Z, roll)
    }
}

impl Default for Rotation {
    fn default() -> Rotation {
        Rotation::IDENTITY
    }
}

impl core::ops::Mul for Rotation {
    type Output = Rotation;
    fn mul(self, r: Rotation) -> Rotation {
        Rotation {
            w: self.w * r.w - self.x * r.x - self.y * r.y - self.z * r.z,
            x: self.w * r.x + self.x * r.w + self.y * r.z - self.z * r.y,
            y: self.w * r.y - self.x * r.z + self.y * r.w + self.z * r.x,
            z: self.w * r.z + self.x * r.y - self.y * r.x + self.z * r.w,
        }
    }
}

/// So a shared `&Rotation` can be handed to entity setters just like the
/// macro-generated vector types (which get this impl from `vectors!`).
impl AsRef<Rotation> for Rotation {
    fn as_ref(&self) -> &Rotation {
        self
    }
}

// Raw-quaternion conversions, in (x, y, z, w) order — the same tuple/array
// round-tripping the vector family gets from `vectors!`. These are for
// callers who already hold quaternion components; they are *not* normalized,
// matching the "explicit opt-out of the type discipline" rule.
impl From<(f64, f64, f64, f64)> for Rotation {
    fn from(q: (f64, f64, f64, f64)) -> Rotation {
        Rotation {
            x: q.0,
            y: q.1,
            z: q.2,
            w: q.3,
        }
    }
}

impl From<[f64; 4]> for Rotation {
    fn from(q: [f64; 4]) -> Rotation {
        Rotation {
            x: q[0],
            y: q[1],
            z: q[2],
            w: q[3],
        }
    }
}

impl From<Rotation> for (f64, f64, f64, f64) {
    fn from(r: Rotation) -> (f64, f64, f64, f64) {
        (r.x, r.y, r.z, r.w)
    }
}

impl From<Rotation> for [f64; 4] {
    fn from(r: Rotation) -> [f64; 4] {
        [r.x, r.y, r.z, r.w]
    }
}

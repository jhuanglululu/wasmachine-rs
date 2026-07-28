//! Known-answer tests for the math layer: macro-generated operators and
//! conversions, plus the hand-written quaternion/angle/duration algebra.
//! Expected values are computed by hand, never with the same formula the
//! code under test uses.

use wasmachine::math::*;

fn approx(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

#[test]
fn position_offset_algebra() {
    let p = Position::new(1.0, 2.0, 3.0);
    let o = Offset::new(4.0, 5.0, 6.0);
    assert_eq!(p + o, Position::new(5.0, 7.0, 9.0));
    assert_eq!(p - o, Position::new(-3.0, -3.0, -3.0));
    assert_eq!(
        Position::new(5.0, 7.0, 9.0) - Position::new(1.0, 2.0, 3.0),
        Offset::new(4.0, 5.0, 6.0)
    );

    let mut q = p;
    q += o;
    assert_eq!(q, Position::new(5.0, 7.0, 9.0));
}

#[test]
fn offset_scaling() {
    assert_eq!(
        Offset::new(1.0, -2.0, 0.5) * 2.0,
        Offset::new(2.0, -4.0, 1.0)
    );
    assert_eq!(
        2.0 * Offset::new(1.0, -2.0, 0.5),
        Offset::new(2.0, -4.0, 1.0)
    );
    assert_eq!(
        Offset::new(2.0, -4.0, 1.0) / 2.0,
        Offset::new(1.0, -2.0, 0.5)
    );
    assert_eq!(-Offset::new(1.0, -2.0, 0.5), Offset::new(-1.0, 2.0, -0.5));
}

#[test]
fn vector_length_dot_cross_normalize() {
    // length: a 3-4-5 right triangle in the xy-plane.
    assert!(approx(Vector3d::new(3.0, 4.0, 0.0).length(), 5.0));
    // Offset length: 2² + 3² + 6² = 49 -> 7.
    assert!(approx(Offset::new(2.0, 3.0, 6.0).length(), 7.0));

    // dot: 1*4 + 2*(-5) + 3*6 = 4 - 10 + 18 = 12.
    assert!(approx(
        Vector3d::new(1.0, 2.0, 3.0).dot(Vector3d::new(4.0, -5.0, 6.0)),
        12.0
    ));
    // Perpendicular vectors dot to zero.
    assert!(approx(Vector3d::X.dot(Vector3d::Y), 0.0));

    // cross: X × Y = Z (right-handed).
    assert_eq!(Vector3d::X.cross(Vector3d::Y), Vector3d::Z);
    // General cross, computed by hand: (2*6-3*5, 3*4-1*6, 1*5-2*4) = (-3, 6, -3).
    assert_eq!(
        Vector3d::new(1.0, 2.0, 3.0).cross(Vector3d::new(4.0, 5.0, 6.0)),
        Vector3d::new(-3.0, 6.0, -3.0)
    );

    // normalize keeps direction, gives unit length.
    let n = Vector3d::new(0.0, 5.0, 0.0).normalize();
    assert_eq!(n, Vector3d::new(0.0, 1.0, 0.0));
    assert!(approx(
        Vector3d::new(3.0, 0.0, 4.0).normalize().length(),
        1.0
    ));
}

#[test]
#[should_panic(expected = "zero-length")]
fn normalize_zero_kills() {
    let _ = Vector3d::ZERO.normalize();
}

#[test]
fn velocity_times_duration_is_offset() {
    let v = Velocity::new(0.5, 0.0, -0.25);
    assert_eq!(v * Ticks::new(40), Offset::new(20.0, 0.0, -10.0));
}

#[test]
fn explicit_conversions_within_f64_family() {
    let p: Position = Vector3d::new(1.0, 2.0, 3.0).into();
    assert_eq!(p, Position::new(1.0, 2.0, 3.0));
    let o: Offset = p.into();
    assert_eq!(o, Offset::new(1.0, 2.0, 3.0));
    let s: Scale = Vector3d::new(1.5, 2.0, 0.5).into();
    assert_eq!(s, Scale::new(1.5, 2.0, 0.5));

    let from_tuple: Position = (1.0, 2.0, 3.0).into();
    assert_eq!(from_tuple, Position::new(1.0, 2.0, 3.0));
    let from_array: Scale = [2.0, 2.0, 2.0].into();
    assert_eq!(from_array, Scale::splat(2.0));
    let back: [f64; 3] = from_tuple.into();
    assert_eq!(back, [1.0, 2.0, 3.0]);
    let tup: (i64, i64, i64) = Vector3i::new(4, 5, 6).into();
    assert_eq!(tup, (4, 5, 6));
}

#[test]
fn explicit_conversions_cross_element() {
    // i64 -> f64 is exact.
    let v: Vector3d = Vector3i::new(1, -2, 3).into();
    assert_eq!(v, Vector3d::new(1.0, -2.0, 3.0));
    let p: Position = Vector3i::new(7, 8, 9).into();
    assert_eq!(p, Position::new(7.0, 8.0, 9.0));

    // f64 -> i64 truncates toward zero (as-cast semantics) — distinct from
    // floor for negatives.
    let t: Vector3i = Position::new(1.7, -2.3, 0.5).into();
    assert_eq!(t, Vector3i::new(1, -2, 0));
}

#[test]
fn rounding_to_block_coords() {
    let v = Vector3d::new(1.7, -2.3, 0.5);
    assert_eq!(v.floor(), Vector3i::new(1, -3, 0));
    assert_eq!(v.ceil(), Vector3i::new(2, -2, 1));
    assert_eq!(v.round(), Vector3i::new(2, -2, 1)); // 0.5 rounds away from zero
}

#[test]
fn scale_composition() {
    assert_eq!(
        Scale::new(2.0, 1.0, 0.5) * Scale::new(3.0, 2.0, 4.0),
        Scale::new(6.0, 2.0, 2.0)
    );
    assert_eq!(Scale::splat(1.0), Scale::new(1.0, 1.0, 1.0));

    // MulAssign composes in place: (2,3,4) * (1,2,0.5) = (2,6,2).
    let mut s = Scale::new(2.0, 3.0, 4.0);
    s *= Scale::new(1.0, 2.0, 0.5);
    assert_eq!(s, Scale::new(2.0, 6.0, 2.0));
}

#[test]
fn ticks_construction_and_seconds() {
    assert_eq!(Ticks::from_secs(2.5), Ticks::new(50));
    assert_eq!(Ticks::ONE_SECOND, Ticks::new(20));
    assert_eq!(Ticks::from(30u64).count(), 30);
    assert!(approx(Ticks::new(30).as_secs(), 1.5));
}

#[test]
fn ticks_arithmetic() {
    assert_eq!(Ticks::new(10) + Ticks::new(5), Ticks::new(15));
    assert_eq!(Ticks::new(10) - Ticks::new(4), Ticks::new(6));
    assert_eq!(Ticks::new(10) * 3, Ticks::new(30));
    assert_eq!(Ticks::new(50) / 5, Ticks::new(10));
    // 50 mod 7: 7*7 = 49, remainder 1.
    assert_eq!(Ticks::new(50) % 7, Ticks::new(1));
    // Ticks / Ticks -> how many fit: 50 / 20 = 2.
    assert_eq!(Ticks::new(50) / Ticks::new(20), 2u64);
    // Ticks % Ticks -> leftover: 50 - 2*20 = 10.
    assert_eq!(Ticks::new(50) % Ticks::new(20), Ticks::new(10));

    let mut t = Ticks::new(100);
    t += Ticks::new(5);
    assert_eq!(t, Ticks::new(105));
    t -= Ticks::new(5);
    assert_eq!(t, Ticks::new(100));
    t *= 2;
    assert_eq!(t, Ticks::new(200));
    t /= 4;
    assert_eq!(t, Ticks::new(50));
    t %= 7;
    assert_eq!(t, Ticks::new(1)); // 50 % 7 = 1
    let mut u = Ticks::new(50);
    u %= Ticks::new(20);
    assert_eq!(u, Ticks::new(10));
}

#[test]
#[should_panic(expected = "negative duration")]
fn negative_duration_kills() {
    let _ = Ticks::from_secs(-0.5);
}

#[test]
#[should_panic]
fn ticks_underflow_kills() {
    // Underflow panics rather than wrapping — a negative duration is a bug.
    let _ = Ticks::new(1) - Ticks::new(2);
}

#[test]
fn angle_conversions() {
    let r: Radians = Degrees::new(180.0).into();
    assert!(approx(r.value(), core::f64::consts::PI));
    let d: Degrees = Radians::new(core::f64::consts::FRAC_PI_2).into();
    assert!(approx(d.value(), 90.0));
    // From<f64> constructors.
    assert!(approx(Degrees::from(45.0).value(), 45.0));
    assert!(approx(Radians::from(1.5).value(), 1.5));
}

#[test]
fn angle_arithmetic() {
    // Degrees.
    assert!(approx(
        (Degrees::new(30.0) + Degrees::new(60.0)).value(),
        90.0
    ));
    assert!(approx(
        (Degrees::new(90.0) - Degrees::new(30.0)).value(),
        60.0
    ));
    assert!(approx((-Degrees::new(45.0)).value(), -45.0));
    assert!(approx((Degrees::new(45.0) * 2.0).value(), 90.0));
    assert!(approx((Degrees::new(90.0) / 2.0).value(), 45.0));

    let mut a = Degrees::new(10.0);
    a += Degrees::new(5.0);
    assert!(approx(a.value(), 15.0));
    a -= Degrees::new(20.0);
    assert!(approx(a.value(), -5.0));
    a *= -2.0;
    assert!(approx(a.value(), 10.0));
    a /= 4.0;
    assert!(approx(a.value(), 2.5));

    // Radians.
    assert!(approx((Radians::new(1.0) + Radians::new(0.5)).value(), 1.5));
    assert!(approx((Radians::new(2.0) * 0.25).value(), 0.5));
}

#[test]
fn axis_angle_known_quaternion() {
    // 90 deg around +Y: (x,y,z,w) = (0, sin 45, 0, cos 45)
    let q = Rotation::axis_angle(Vector3d::Y, Degrees::new(90.0));
    let s = core::f64::consts::FRAC_1_SQRT_2;
    assert!(approx(q.x, 0.0));
    assert!(approx(q.y, s));
    assert!(approx(q.z, 0.0));
    assert!(approx(q.w, s));
}

#[test]
fn axis_angle_normalizes_axis() {
    let a = Rotation::axis_angle(Vector3d::new(0.0, 2.0, 0.0), Degrees::new(90.0));
    let b = Rotation::axis_angle(Vector3d::Y, Degrees::new(90.0));
    assert!(approx(a.x, b.x) && approx(a.y, b.y) && approx(a.z, b.z) && approx(a.w, b.w));
}

#[test]
fn quaternion_composition() {
    // Two 90-deg rotations around Y compose to 180 deg: (0, 1, 0, 0).
    let q = Rotation::axis_angle(Vector3d::Y, Degrees::new(90.0));
    let qq = q * q;
    assert!(approx(qq.x, 0.0));
    assert!(approx(qq.y, 1.0));
    assert!(approx(qq.z, 0.0));
    assert!(approx(qq.w, 0.0));

    // Identity is neutral.
    let i = Rotation::IDENTITY * q;
    assert!(approx(i.y, q.y) && approx(i.w, q.w));
}

#[test]
fn euler_matches_axis_angle_for_pure_yaw() {
    let e = Rotation::euler(Degrees::new(90.0), Degrees::new(0.0), Degrees::new(0.0));
    let a = Rotation::axis_angle(Vector3d::Y, Degrees::new(90.0));
    assert!(approx(e.x, a.x) && approx(e.y, a.y) && approx(e.z, a.z) && approx(e.w, a.w));
}

#[test]
fn rotation_raw_conversions() {
    // (x, y, z, w) order, no normalization applied.
    let from_tuple: Rotation = (0.1, 0.2, 0.3, 0.4).into();
    assert_eq!(
        from_tuple,
        Rotation {
            x: 0.1,
            y: 0.2,
            z: 0.3,
            w: 0.4
        }
    );
    let from_array: Rotation = [1.0, 0.0, 0.0, 0.0].into();
    assert_eq!(
        from_array,
        Rotation {
            x: 1.0,
            y: 0.0,
            z: 0.0,
            w: 0.0
        }
    );

    // Round-trip back out to tuple/array; identity is (0, 0, 0, 1).
    let tup: (f64, f64, f64, f64) = Rotation::IDENTITY.into();
    assert_eq!(tup, (0.0, 0.0, 0.0, 1.0));
    let arr: [f64; 4] = Rotation::IDENTITY.into();
    assert_eq!(arr, [0.0, 0.0, 0.0, 1.0]);

    // A computed rotation survives an array round-trip unchanged.
    let q = Rotation::axis_angle(Vector3d::Y, Degrees::new(90.0));
    let back: Rotation = <[f64; 4]>::from(q).into();
    assert_eq!(q, back);
}

#[test]
fn unit_constants() {
    assert_eq!(Vector3d::X, Vector3d::new(1.0, 0.0, 0.0));
    assert_eq!(Vector3i::Z, Vector3i::new(0, 0, 1));
    assert_eq!(Position::ZERO, Position::new(0.0, 0.0, 0.0));
}

#[test]
fn math_params_take_as_ref() {
    // Both an owned value and a shared reference satisfy the SDK setters'
    // `impl AsRef<T>` params (the blanket `&T: AsRef<T>` covers the borrow).
    fn takes(p: impl AsRef<Position>) -> Position {
        *p.as_ref()
    }
    let p = Position::new(1.0, 2.0, 3.0);
    let p_ref: &Position = &p;
    assert_eq!(takes(p), p);
    assert_eq!(takes(p_ref), p);

    fn takes_rot(r: impl AsRef<Rotation>) -> Rotation {
        *r.as_ref()
    }
    let id_ref: &Rotation = &Rotation::IDENTITY;
    assert_eq!(takes_rot(Rotation::IDENTITY), Rotation::IDENTITY);
    assert_eq!(takes_rot(id_ref), Rotation::IDENTITY);
}

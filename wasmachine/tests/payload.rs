//! Channel-payload contract tests: the core types that claim to be `Pod` must
//! actually survive a byte round trip, and the sync handles must be `Send`
//! (movable into a `spawn`/`scope` closure) with the receiver deliberately not
//! clonable.
//!
//! The host stubs panic for anything that crosses the ABI, so these tests
//! check the *type-level* contract — which is exactly what the ABI cannot
//! check for us. (A plugin SDK's own payload types — its colors, its
//! `payload!` macro — are tested in that SDK's suite.)

use bytemuck::{Pod, Zeroable};
use wasmachine::math::{Degrees, Offset, Position, Radians, Rotation, Scale, Ticks, Vector3i};
use wasmachine::sync::{Barrier, Composite, Receiver, Sender, Signal};

fn requires_sync<T: Sync>() {}
fn requires_send<T: Send>() {}
fn requires_clone<T: Clone>() {}
fn requires_pod<T: Pod>() {}

#[test]
fn sync_handles_can_cross_into_a_spawned_task() {
    // `spawn`'s closure is `Send + 'static` (`Scope::spawn`'s is `Send +
    // 'scope`): every sync handle must be `Send`, whatever the payload type is.
    requires_send::<Signal>();
    requires_send::<Barrier>();
    requires_send::<Composite>();
    requires_send::<Sender<Position>>();
    requires_send::<Receiver<Position>>();
    // They are `Sync` too — a plain host id has nothing to make it otherwise —
    // so a handle can also be captured by reference.
    requires_sync::<Signal>();
    requires_sync::<Barrier>();
    requires_sync::<Composite>();
    requires_sync::<Sender<Position>>();
    requires_sync::<Receiver<Position>>();

    // Senders clone (one per producer); the receiver deliberately does not,
    // which is what keeps the channel single-consumer.
    requires_clone::<Signal>();
    requires_clone::<Barrier>();
    requires_clone::<Sender<Position>>();
}

#[test]
fn math_types_are_channel_payloads() {
    requires_pod::<Position>();
    requires_pod::<Offset>();
    requires_pod::<Scale>();
    requires_pod::<Vector3i>();
    requires_pod::<Rotation>();
    requires_pod::<Ticks>();
    requires_pod::<Degrees>();
    requires_pod::<Radians>();
}

#[test]
fn math_types_round_trip_through_bytes() {
    let p = Position::new(1.5, -2.25, 3.0);
    let bytes = bytemuck::bytes_of(&p);
    assert_eq!(bytes.len(), 24); // three f64, no padding
    assert_eq!(*bytemuck::from_bytes::<Position>(bytes), p);

    let v = Vector3i::new(-1, 2, i64::MAX);
    assert_eq!(*bytemuck::from_bytes::<Vector3i>(bytemuck::bytes_of(&v)), v);

    let r = Rotation::axis_angle(wasmachine::math::Vector3d::Y, Degrees::new(90.0));
    let bytes = bytemuck::bytes_of(&r);
    assert_eq!(bytes.len(), 32); // four f64
    assert_eq!(*bytemuck::from_bytes::<Rotation>(bytes), r);

    let t = Ticks::new(40);
    assert_eq!(bytemuck::bytes_of(&t).len(), 8);
    assert_eq!(*bytemuck::from_bytes::<Ticks>(bytemuck::bytes_of(&t)), t);
}

/// The shape a plugin SDK's `payload!` macro produces, written out by hand
/// here: a `#[repr(C)]` struct of core types deriving `Pod`/`Zeroable`. The
/// SDK's version routes the derives through its own bytemuck re-export so the
/// animation needs no bytemuck dependency; the layout guarantee under test is
/// the same one.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
struct Waypoint {
    target: Position,
    over: Ticks,
}

#[test]
fn a_payload_struct_of_core_types_round_trips() {
    requires_pod::<Waypoint>();
    requires_send::<Sender<Waypoint>>();

    let w = Waypoint {
        target: Position::new(1.0, 2.0, 3.0),
        over: Ticks::new(12),
    };
    // repr(C), so the layout is exactly the fields in order: 24 + 8.
    let bytes = bytemuck::bytes_of(&w);
    assert_eq!(bytes.len(), 32);
    assert_eq!(*bytemuck::from_bytes::<Waypoint>(bytes), w);
    // Derived Copy/Debug/PartialEq come along too.
    let copy = w;
    assert_eq!(copy, w);
    // And Zeroable, for the receive buffer.
    assert_eq!(Waypoint::zeroed().over, Ticks::new(0));
    assert_eq!(Waypoint::zeroed().target, Position::ZERO);
}

//! Event delivery semantics, driven the same way the scheduler drives them: a system's
//! `EventRx` state is created once and fetched every frame, and the `clear_events` system
//! calls `Events::update` at the end of each frame.
//!
//! Every operation goes through `ready`, so a blocked send shows up as a failure rather than a hang.
//!
//! Tests marked `#[ignore = "known issue: ..."]` assert the intended behavior and currently fail.
//! Run them with `cargo test -p weaver-event -- --ignored`.

use futures_util::{FutureExt, StreamExt};
use weaver_ecs::{system::SystemParam, world::World};
use weaver_event::{EventRx, EventRxState, EventTx, Events};

fn ready<F: Future>(future: F) -> F::Output {
    future
        .now_or_never()
        .expect("operation blocked: the event channel is full")
}

fn world_with_events() -> World {
    let world = World::new();
    world.insert_resource(Events::<u32>::new());
    world
}

fn send(world: &World, event: u32) {
    let mut tx = <EventTx<u32> as SystemParam>::fetch(world, &());
    ready(tx.send(event));
}

fn read(world: &World, state: &EventRxState<u32>) -> Vec<u32> {
    let rx = <EventRx<u32> as SystemParam>::fetch(world, state);
    ready(rx.collect())
}

fn end_frame(world: &World) {
    let mut events = world.get_resource_mut::<Events<u32>>().unwrap();
    ready(events.update(world.read_change_tick()));
}

/// A reader that runs every frame must receive each event exactly once.
#[test]
#[ignore = "known issue: EventRx reads from a clone of a state receiver that never advances"]
fn each_event_is_delivered_once_per_reader() {
    let world = world_with_events();
    let state = <EventRx<u32> as SystemParam>::init_state(&world);

    let mut received = Vec::new();
    for frame in 0..4 {
        send(&world, frame);
        received.extend(read(&world, &state));
        end_frame(&world);
    }

    assert_eq!(received, vec![0, 1, 2, 3]);
}

/// Sending one event per frame must keep working indefinitely while a reader is present.
#[test]
#[ignore = "known issue: the reader's state receiver never advances, so the 1024-slot channel fills"]
fn reader_does_not_block_sender_over_many_frames() {
    let world = world_with_events();
    let state = <EventRx<u32> as SystemParam>::init_state(&world);

    for frame in 0..2000 {
        send(&world, frame);
        read(&world, &state);
        end_frame(&world);
    }
}

/// `Events<T>` inserted without a clear system (as `add_asset_loader` does for `AssetLoaded<T>`)
/// must not stop accepting events after a fixed number of sends.
#[test]
#[ignore = "known issue: Events holds its own receiver, which only Events::update drains"]
fn sending_without_a_clear_system_never_blocks() {
    let world = World::new();
    world.init_resource::<Events<u32>>();

    for event in 0..2000 {
        let events = world.get_resource::<Events<u32>>().unwrap();
        ready(events.send(event));
    }
}

/// More events than the channel capacity can be sent in one frame and are all delivered.
#[test]
#[ignore = "known issue: events are stored in a bounded channel (capacity 1024) that blocks when full"]
fn burst_larger_than_channel_capacity_is_delivered() {
    let world = world_with_events();
    let state = <EventRx<u32> as SystemParam>::init_state(&world);

    for event in 0..2000 {
        send(&world, event);
    }
    let received = read(&world, &state);

    assert_eq!(received.len(), 2000);
}

//! Event delivery semantics, driven the same way the scheduler drives them: a system's
//! `EventRx` state is created once and fetched every frame, and the `clear_events` system
//! calls `EventQueue::update` at the end of each frame.

use weaver_ecs::{system::SystemParam, world::World};
use weaver_event::{EventQueue, EventRx, EventRxState, EventTx};

fn world_with_events() -> World {
    let world = World::new();
    world.insert_resource(EventQueue::<u32>::new());
    world
}

fn send(world: &World, event: u32) {
    let mut tx = <EventTx<u32> as SystemParam>::fetch(world, &());
    tx.send(event);
}

fn reader(world: &World) -> EventRxState<u32> {
    <EventRx<u32> as SystemParam>::init_state(world)
}

fn read(world: &World, state: &EventRxState<u32>) -> Vec<u32> {
    <EventRx<u32> as SystemParam>::fetch(world, state).collect()
}

fn end_frame(world: &World) {
    world
        .get_resource_mut::<EventQueue<u32>>()
        .unwrap()
        .update();
}

/// A reader that runs every frame must receive each event exactly once.
#[test]
fn each_event_is_delivered_once_per_reader() {
    let world = world_with_events();
    let state = reader(&world);

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
fn reader_does_not_block_sender_over_many_frames() {
    let world = world_with_events();
    let state = reader(&world);

    for frame in 0..2000 {
        send(&world, frame);
        assert_eq!(read(&world, &state), vec![frame]);
        end_frame(&world);
    }
}

/// An `EventQueue<T>` with no clear system (as `add_asset_loader` sets up for `AssetLoaded<T>`)
/// must not stop accepting events after a fixed number of sends.
#[test]
fn sending_without_a_clear_system_never_blocks() {
    let world = World::new();
    world.init_resource::<EventQueue<u32>>();

    for event in 0..2000 {
        let mut events = world.get_resource_mut::<EventQueue<u32>>().unwrap();
        events.send(event);
    }
}

/// Any number of events can be sent in one frame, and all of them are delivered.
#[test]
fn burst_of_events_is_delivered() {
    let world = world_with_events();
    let state = reader(&world);

    for event in 0..2000 {
        send(&world, event);
    }
    let received = read(&world, &state);

    assert_eq!(received.len(), 2000);
}

/// A reader that runs before the sender each frame gets the previous frame's events from the
/// back buffer, still exactly once each.
#[test]
fn reader_before_sender_gets_events_next_frame() {
    let world = world_with_events();
    let state = reader(&world);

    let mut received = Vec::new();
    for frame in 0..4 {
        received.push(read(&world, &state));
        send(&world, frame);
        end_frame(&world);
    }

    assert_eq!(received, vec![vec![], vec![0], vec![1], vec![2]]);
}

/// Every reader has its own cursor, so one reader consuming events doesn't hide them from another.
#[test]
fn readers_are_independent() {
    let world = world_with_events();
    let first = reader(&world);
    let second = reader(&world);

    send(&world, 1);
    assert_eq!(read(&world, &first), vec![1]);
    send(&world, 2);

    assert_eq!(read(&world, &first), vec![2]);
    assert_eq!(read(&world, &second), vec![1, 2]);
}

/// Events a reader leaves unread are still there on its next run.
#[test]
fn unread_events_carry_over_to_the_next_run() {
    let world = world_with_events();
    let state = reader(&world);

    send(&world, 1);
    send(&world, 2);
    let mut rx = <EventRx<u32> as SystemParam>::fetch(&world, &state);
    assert_eq!(rx.next(), Some(1));
    drop(rx);
    end_frame(&world);

    assert_eq!(read(&world, &state), vec![2]);
}

/// An event survives two updates and is dropped by the third, so a reader that skips two frames
/// misses it and picks up from the oldest event still buffered.
#[test]
fn events_are_dropped_after_two_updates() {
    let world = world_with_events();
    let state = reader(&world);

    send(&world, 1);
    end_frame(&world);
    send(&world, 2);
    end_frame(&world);
    send(&world, 3);

    assert_eq!(read(&world, &state), vec![2, 3]);
}

/// A reader's state isn't tied to the world it was created in. `Extract<EventRx<T>>` creates it
/// in the render world and reads the main world's queue with it.
#[test]
fn reader_state_works_against_another_world() {
    let render_world = world_with_events();
    let state = reader(&render_world);

    let main_world = world_with_events();
    send(&main_world, 7);

    assert_eq!(read(&main_world, &state), vec![7]);
}

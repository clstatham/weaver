use std::{
    marker::PhantomData,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use weaver_ecs::{
    change_detection::TickRange,
    component::{Res, ResMut},
    system::{SystemAccess, SystemParam},
    world::World,
};

pub mod prelude {
    pub use super::{Event, EventQueue, EventRx, EventTx};
}

pub trait Event: 'static + Send + Sync + Clone {}
impl<T: 'static + Send + Sync + Clone> Event for T {}

/// Double-buffered storage for events of type `T`.
///
/// Events are sent into the front buffer. [`EventQueue::update`] (run once per frame by the system
/// that `add_event` registers) moves them to the back buffer and drops the events that were there,
/// so an event stays readable for two updates. A reader that runs before the sender in a frame
/// therefore still sees the event on its next run.
///
/// Every event has a sequential id, its position in the stream of all events ever sent. Readers
/// never remove events; each one keeps a cursor holding the id of the next event it hasn't read, so
/// any number of readers can read the same events independently.
pub struct EventQueue<T: Event> {
    /// Events sent since the last update.
    front_buffer: Vec<T>,
    /// Events sent between the two most recent updates.
    back_buffer: Vec<T>,
    /// How many events have ever been sent, which is also the id of the next one.
    event_count: usize,
}

impl<T: Event> Default for EventQueue<T> {
    fn default() -> Self {
        Self {
            front_buffer: Vec::new(),
            back_buffer: Vec::new(),
            event_count: 0,
        }
    }
}

impl<T: Event> EventQueue<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn send(&mut self, event: T) {
        self.front_buffer.push(event);
        self.event_count += 1;
    }

    pub fn send_default(&mut self)
    where
        T: Default,
    {
        self.send(T::default());
    }

    /// Moves this frame's events to the back buffer, dropping the ones sent before the previous update.
    pub fn update(&mut self) {
        // reuse the old back buffer's allocation for the next frame's events
        std::mem::swap(&mut self.front_buffer, &mut self.back_buffer);
        self.front_buffer.clear();
    }

    /// Drops every buffered event. Ids keep counting up, so readers' cursors stay valid.
    pub fn clear(&mut self) {
        self.front_buffer.clear();
        self.back_buffer.clear();
    }

    /// The number of buffered events, read or not.
    pub fn len(&self) -> usize {
        self.back_buffer.len() + self.front_buffer.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The id of the oldest buffered event. Both buffers hold consecutive ids, the back buffer's first.
    fn oldest_id(&self) -> usize {
        self.event_count - self.len()
    }

    /// Returns the event with the given id, if it's still buffered.
    fn get(&self, id: usize) -> Option<&T> {
        let index = id.checked_sub(self.oldest_id())?;
        match index.checked_sub(self.back_buffer.len()) {
            None => self.back_buffer.get(index),
            Some(index) => self.front_buffer.get(index),
        }
    }
}

/// Sends events into an [`EventQueue`].
pub struct EventTx<T: Event> {
    events: ResMut<EventQueue<T>>,
}

impl<T: Event> EventTx<T> {
    pub fn new(events: ResMut<EventQueue<T>>) -> Self {
        Self { events }
    }

    pub fn send(&mut self, event: T) {
        self.events.send(event);
    }
}

/// A reader's cursor into an [`EventQueue`]: the id of the next event it hasn't read.
///
/// Clones share the cursor, so reading through the `EventRx` a system gets each run advances the
/// state the system keeps between runs.
pub struct EventRxState<T: Event> {
    next_id: Arc<AtomicUsize>,
    _marker: PhantomData<fn() -> T>,
}

impl<T: Event> Default for EventRxState<T> {
    fn default() -> Self {
        Self {
            next_id: Arc::new(AtomicUsize::new(0)),
            _marker: PhantomData,
        }
    }
}

impl<T: Event> Clone for EventRxState<T> {
    fn clone(&self) -> Self {
        Self {
            next_id: self.next_id.clone(),
            _marker: PhantomData,
        }
    }
}

/// Iterates over the events in an [`EventQueue`] that this reader hasn't read yet, oldest first.
///
/// Events left unread are still there on the next run, unless the queue has dropped them by then:
/// a reader that goes two updates without running misses the events that were sent in between.
pub struct EventRx<T: Event> {
    events: Res<EventQueue<T>>,
    state: EventRxState<T>,
}

impl<T: Event> EventRx<T> {
    pub fn new(events: Res<EventQueue<T>>, state: EventRxState<T>) -> Self {
        Self { events, state }
    }

    /// The id of the next event to read, skipping any the queue has already dropped.
    fn next_id(&self) -> usize {
        self.state
            .next_id
            .load(Ordering::Relaxed)
            .max(self.events.oldest_id())
    }

    /// The number of events this reader hasn't read yet.
    pub fn len(&self) -> usize {
        self.events.event_count.saturating_sub(self.next_id())
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<T: Event> Iterator for EventRx<T> {
    type Item = T;

    fn next(&mut self) -> Option<T> {
        let id = self.next_id();
        let event = self.events.get(id)?.clone();
        self.state.next_id.store(id + 1, Ordering::Relaxed);
        Some(event)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl<T: Event> SystemParam for EventTx<T> {
    type Item = EventTx<T>;
    type State = ();

    fn access() -> SystemAccess {
        <ResMut<EventQueue<T>> as SystemParam>::access()
    }

    fn init_state(_world: &World) -> Self::State {}

    fn fetch(world: &World, _state: &(), ticks: TickRange) -> Self::Item {
        EventTx::new(<ResMut<EventQueue<T>> as SystemParam>::fetch(
            world,
            &(),
            ticks,
        ))
    }

    fn can_run(world: &World) -> bool {
        <ResMut<EventQueue<T>> as SystemParam>::can_run(world)
    }
}

impl<T: Event> SystemParam for EventRx<T> {
    type Item = EventRx<T>;
    type State = EventRxState<T>;

    fn access() -> SystemAccess {
        <Res<EventQueue<T>> as SystemParam>::access()
    }

    // The cursor starts at 0 rather than at the queue's current count, so a new reader sees whatever
    // is still buffered, and the state doesn't depend on the world it was created in
    // (`Extract<EventRx<T>>` creates it in the render world but reads the main world's queue).
    fn init_state(_world: &World) -> Self::State {
        EventRxState::default()
    }

    fn fetch(world: &World, state: &Self::State, ticks: TickRange) -> Self::Item {
        EventRx::new(
            <Res<EventQueue<T>> as SystemParam>::fetch(world, &(), ticks),
            state.clone(),
        )
    }

    fn can_run(world: &World) -> bool {
        <Res<EventQueue<T>> as SystemParam>::can_run(world)
    }
}

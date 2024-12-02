use std::{
    any::TypeId,
    collections::VecDeque,
    ops::{Deref, DerefMut},
};

use weaver_ecs::{
    system::{SystemAccess, SystemParam},
    world::{Tick, World},
};
use weaver_util::{
    lock::{Read, SharedLock},
    FxHashSet,
};

pub mod prelude {
    pub use super::{Event, EventRx, EventTx};
}

pub trait Event: 'static + Send + Sync {}

pub struct EventRef<'a, T: Event> {
    events: Read<'a, VecDeque<T>>,
    index: usize,
}

impl<'a, T: Event> Deref for EventRef<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.events[self.index]
    }
}

pub struct Events<T: Event> {
    front_buffer: SharedLock<VecDeque<T>>,
    back_buffer: SharedLock<VecDeque<T>>,
    updated_tick: SharedLock<Tick>,
}

impl<T: Event> Default for Events<T> {
    fn default() -> Self {
        Self {
            front_buffer: SharedLock::new(VecDeque::new()),
            back_buffer: SharedLock::new(VecDeque::new()),
            updated_tick: SharedLock::new(Tick::default()),
        }
    }
}

impl<T: Event> Clone for Events<T> {
    fn clone(&self) -> Self {
        Self {
            front_buffer: self.front_buffer.clone(),
            back_buffer: self.back_buffer.clone(),
            updated_tick: self.updated_tick.clone(),
        }
    }
}

impl<T: Event> Events<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&self) {
        self.front_buffer.write().clear();
        self.back_buffer.write().clear();
    }

    pub fn update(&self, change_tick: Tick) {
        *self.updated_tick.write() = change_tick;
        self.back_buffer.write().clear();
        self.back_buffer
            .write()
            .extend(&mut self.front_buffer.write().drain(..));
    }

    pub fn send(&self, event: T) {
        self.front_buffer.write().push_back(event);
    }

    pub fn send_default(&self)
    where
        T: Default,
    {
        self.send(T::default());
    }
}

pub struct EventTx<T: Event> {
    events: Events<T>,
}

impl<T: Event> EventTx<T> {
    pub fn new(events: Events<T>) -> Self {
        Self { events }
    }

    pub fn send(&mut self, event: T) {
        self.events.send(event);
    }
}

pub struct EventRx<T: Event> {
    events: Events<T>,
    include_back_buffer: bool,
}

impl<T: Event> EventRx<T> {
    pub fn new(events: Events<T>, last_run: Tick, this_run: Tick) -> Self {
        // if the tick that the events were last updated on, older than the tick that the system is running on, then include the back buffer
        let include_back_buffer = !events.updated_tick.read().is_newer_than(last_run, this_run);
        Self {
            include_back_buffer,
            events,
        }
    }

    pub fn len(&self) -> usize {
        if self.include_back_buffer {
            self.events.front_buffer.read().len() + self.events.back_buffer.read().len()
        } else {
            self.events.front_buffer.read().len()
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn clear(&self) {
        self.events.clear();
    }

    pub fn iter(&self) -> EventIter<'_, T> {
        EventIter {
            events: &self.events,
            unread: self.len(),
            index: 0,
            include_back_buffer: self.include_back_buffer,
            _marker: std::marker::PhantomData,
        }
    }
}

pub struct EventIter<'a, T: Event> {
    events: &'a Events<T>,
    unread: usize,
    index: usize,
    include_back_buffer: bool,
    _marker: std::marker::PhantomData<&'a ()>,
}

impl<'a, T: Event> Iterator for EventIter<'a, T> {
    type Item = EventRef<'a, T>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.unread == 0 {
            return None;
        }

        let event = if self.include_back_buffer {
            if self.index < self.events.front_buffer.read().len() {
                EventRef {
                    events: self.events.front_buffer.read(),
                    index: self.index,
                }
            } else {
                EventRef {
                    events: self.events.back_buffer.read(),
                    index: self.index - self.events.front_buffer.read().len(),
                }
            }
        } else {
            EventRef {
                events: self.events.front_buffer.read(),
                index: self.index,
            }
        };

        self.index += 1;
        self.unread -= 1;

        Some(event)
    }
}

impl<T: Event> SystemParam for EventTx<T> {
    type Item = EventTx<T>;
    type State = Events<T>;

    fn access() -> SystemAccess {
        SystemAccess {
            exclusive: true,
            resources_written: FxHashSet::from_iter([TypeId::of::<Events<T>>()]),
            ..Default::default()
        }
    }

    fn init_state(world: &World) -> Self::State {
        world
            .get_resource::<Events<T>>()
            .expect("Events resource not found")
            .clone()
    }

    fn fetch(_world: &World, state: &Events<T>) -> Self::Item {
        EventTx::new(state.clone())
    }
}

impl<T: Event> SystemParam for EventRx<T> {
    type Item = EventRx<T>;
    type State = ();

    fn access() -> SystemAccess {
        SystemAccess {
            exclusive: false,
            resources_read: FxHashSet::from_iter([TypeId::of::<Events<T>>()]),
            ..Default::default()
        }
    }

    fn init_state(_world: &World) -> Self::State {}

    fn fetch(world: &World, _state: &Self::State) -> Self::Item {
        if let Some(events) = world.get_resource::<Events<T>>() {
            EventRx::new(
                events.clone(),
                world.last_change_tick(),
                world.read_change_tick(),
            )
        } else {
            panic!("Events resource not found");
        }
    }
}

pub struct ManuallyUpdatedEvents<T: Event> {
    pub events: Events<T>,
}

impl<T: Event> ManuallyUpdatedEvents<T> {
    pub fn new(events: Events<T>) -> Self {
        Self { events }
    }
}

impl<T: Event> Deref for ManuallyUpdatedEvents<T> {
    type Target = Events<T>;

    fn deref(&self) -> &Self::Target {
        &self.events
    }
}

impl<T: Event> DerefMut for ManuallyUpdatedEvents<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.events
    }
}

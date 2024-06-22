use std::{any::TypeId, collections::VecDeque, ops::Deref};

use weaver_ecs::{
    change::Tick,
    component::{Res, ResMut},
    prelude::Resource,
    system::{SystemAccess, SystemParam},
    world::WorldLock,
};
use weaver_util::lock::{ArcRead, SharedLock};

pub mod prelude {
    pub use super::{Event, EventRx, EventTx};
}

pub trait Event: 'static + Send + Sync {}

pub struct EventRef<T: Event> {
    events: ArcRead<VecDeque<T>>,
    index: usize,
}

impl<T: Event> Deref for EventRef<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.events[self.index]
    }
}

#[derive(Resource)]
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
        self.back_buffer.write().clear();
        self.back_buffer
            .write()
            .extend(&mut self.front_buffer.write().drain(..));
        *self.updated_tick.write() = change_tick;
    }

    pub fn send(&self, event: T) {
        self.front_buffer.write().push_back(event);
    }
}

pub struct EventTx<T: Event> {
    events: ResMut<Events<T>>,
}

impl<T: Event> EventTx<T> {
    pub fn new(events: ResMut<Events<T>>) -> Self {
        Self { events }
    }

    pub fn send(&mut self, event: T) {
        self.events.send(event);
    }
}

pub struct EventRx<T: Event> {
    events: Res<Events<T>>,
    include_back_buffer: bool,
}

impl<T: Event> EventRx<T> {
    pub fn new(events: Res<Events<T>>, last_run: Tick, this_run: Tick) -> Self {
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
    events: &'a Res<Events<T>>,
    unread: usize,
    index: usize,
    include_back_buffer: bool,
    _marker: std::marker::PhantomData<&'a ()>,
}

impl<'a, T: Event> Iterator for EventIter<'a, T> {
    type Item = EventRef<T>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.unread == 0 {
            return None;
        }

        let event = if self.include_back_buffer {
            if self.index < self.events.front_buffer.read().len() {
                EventRef {
                    events: self.events.front_buffer.read_arc(),
                    index: self.index,
                }
            } else {
                EventRef {
                    events: self.events.back_buffer.read_arc(),
                    index: self.index - self.events.front_buffer.read().len(),
                }
            }
        } else {
            EventRef {
                events: self.events.front_buffer.read_arc(),
                index: self.index,
            }
        };

        self.index += 1;
        self.unread -= 1;

        Some(event)
    }
}

impl<T: Event> SystemParam for EventTx<T> {
    type State = ();
    type Fetch<'w, 's> = Self;

    fn access() -> SystemAccess {
        SystemAccess {
            exclusive: false,
            resources_read: Vec::new(),
            resources_written: vec![TypeId::of::<Events<T>>()],
            components_read: Vec::new(),
            components_written: Vec::new(),
        }
    }

    fn init_state(_: &WorldLock) -> Self::State {}

    fn fetch<'w, 's>(_: &'s mut Self::State, world: &WorldLock) -> Self::Fetch<'w, 's> {
        EventTx::new(world.get_resource_mut::<Events<T>>().unwrap())
    }
}

impl<T: Event> SystemParam for EventRx<T> {
    type State = ();
    type Fetch<'w, 's> = Self;

    fn init_state(_: &WorldLock) -> Self::State {}

    fn access() -> SystemAccess {
        SystemAccess {
            exclusive: false,
            resources_read: Vec::new(),
            resources_written: vec![TypeId::of::<Events<T>>()],
            components_read: Vec::new(),
            components_written: Vec::new(),
        }
    }

    fn fetch<'w, 's>(_: &'s mut Self::State, world: &WorldLock) -> Self::Fetch<'w, 's> {
        EventRx::new(
            world.get_resource::<Events<T>>().unwrap(),
            world.read().last_change_tick(),
            world.read().read_change_tick(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use weaver_ecs::prelude::*;
    use weaver_util::prelude::Result;

    #[derive(Debug, PartialEq)]
    struct TestEvent(i32);

    impl Event for TestEvent {}

    struct First;
    impl SystemStage for First {}

    struct Before;
    impl SystemStage for Before {}

    struct After;
    impl SystemStage for After {}

    fn sender_system(mut event_tx: EventTx<TestEvent>) -> Result<()> {
        event_tx.send(TestEvent(1));
        event_tx.send(TestEvent(2));
        event_tx.send(TestEvent(3));
        Ok(())
    }

    fn receiver_system(event_rx: EventRx<TestEvent>) -> Result<()> {
        let mut iter = event_rx.iter();
        assert_eq!(*iter.next().unwrap(), TestEvent(1));
        assert_eq!(*iter.next().unwrap(), TestEvent(2));
        assert_eq!(*iter.next().unwrap(), TestEvent(3));
        assert!(iter.next().is_none());
        Ok(())
    }

    fn update_events_system(events: Res<Events<TestEvent>>, mut world: WriteWorld) -> Result<()> {
        world.increment_change_tick();
        events.update(world.read_change_tick());
        Ok(())
    }

    #[test]
    fn test_event() {
        let world = World::new().into_world_lock();
        world.insert_resource(Events::<TestEvent>::new());

        let mut event_tx = EventTx::<TestEvent>::fetch(&mut (), &world);

        event_tx.send(TestEvent(1));
        event_tx.send(TestEvent(2));
        event_tx.send(TestEvent(3));
        drop(event_tx);

        let event_rx = EventRx::<TestEvent>::fetch(&mut (), &world);

        let mut iter = event_rx.iter();
        assert_eq!(*iter.next().unwrap(), TestEvent(1));
        assert_eq!(*iter.next().unwrap(), TestEvent(2));
        assert_eq!(*iter.next().unwrap(), TestEvent(3));
        assert!(iter.next().is_none());
    }

    #[test]
    fn test_event_update() {
        let world = World::new().into_world_lock();
        world.insert_resource(Events::<TestEvent>::new());

        let mut event_tx = EventTx::<TestEvent>::fetch(&mut (), &world);

        event_tx.send(TestEvent(1));
        event_tx.send(TestEvent(2));
        event_tx.send(TestEvent(3));
        drop(event_tx);

        let event_rx = EventRx::<TestEvent>::fetch(&mut (), &world);
        let mut iter = event_rx.iter();
        assert_eq!(*iter.next().unwrap(), TestEvent(1));
        assert_eq!(*iter.next().unwrap(), TestEvent(2));
        assert_eq!(*iter.next().unwrap(), TestEvent(3));
        assert!(iter.next().is_none());
        drop(event_rx);

        let event_rx = EventRx::<TestEvent>::fetch(&mut (), &world);
        let mut iter = event_rx.iter();
        assert_eq!(*iter.next().unwrap(), TestEvent(1));
        assert_eq!(*iter.next().unwrap(), TestEvent(2));
        assert_eq!(*iter.next().unwrap(), TestEvent(3));
        assert!(iter.next().is_none());
        drop(event_rx);

        world.write().increment_change_tick();

        world
            .get_resource::<Events<TestEvent>>()
            .unwrap()
            .update(world.read().read_change_tick());

        let event_rx = EventRx::<TestEvent>::fetch(&mut (), &world);

        let mut iter = event_rx.iter();
        assert!(iter.next().is_none());
    }

    #[test]
    fn test_event_system_send_before_receive() {
        let world = World::new().into_world_lock();
        world.insert_resource(Events::<TestEvent>::new());
        world.write().push_update_stage::<First>();
        world.write().push_update_stage::<Before>();
        world.write().push_update_stage::<After>();

        world.write().add_system(update_events_system, First);
        world.write().add_system(sender_system, Before);
        world.write().add_system(receiver_system, After);

        world.update().unwrap();
        world.update().unwrap();
        world.update().unwrap();
    }

    #[test]
    fn test_event_system_receive_before_send() {
        let world = World::new().into_world_lock();
        world.insert_resource(Events::<TestEvent>::new());
        world.write().push_update_stage::<First>();
        world.write().push_update_stage::<Before>();
        world.write().push_update_stage::<After>();

        world.write().add_system(update_events_system, First);
        world.write().add_system(receiver_system, After);
        world.write().add_system(sender_system, Before);

        world.update().unwrap();
        world.update().unwrap();
        world.update().unwrap();
    }
}

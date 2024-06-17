use std::{any::TypeId, collections::VecDeque, ops::Deref};

use weaver_ecs::{
    component::{Res, ResMut},
    prelude::Resource,
    system::{SystemAccess, SystemParam},
    world::World,
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
    events: SharedLock<VecDeque<T>>,
}

impl<T: Event> Default for Events<T> {
    fn default() -> Self {
        Self {
            events: SharedLock::new(VecDeque::new()),
        }
    }
}

impl<T: Event> Clone for Events<T> {
    fn clone(&self) -> Self {
        Self {
            events: self.events.clone(),
        }
    }
}

impl<T: Event> Events<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&self) {
        self.events.write().clear();
    }

    pub fn send(&self, event: T) {
        self.events.write().push_back(event);
    }

    pub fn iter(&self) -> EventIter<'_, T> {
        EventIter {
            events: self.events.clone(),
            unread: self.events.read().len(),
            index: 0,
            _marker: std::marker::PhantomData,
        }
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
}

impl<T: Event> EventRx<T> {
    pub fn new(events: Res<Events<T>>) -> Self {
        Self { events }
    }

    pub fn len(&self) -> usize {
        self.events.events.read().len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.events.read().is_empty()
    }

    pub fn iter(&self) -> EventIter<'_, T> {
        self.events.iter()
    }
}

pub struct EventIter<'a, T: Event> {
    events: SharedLock<VecDeque<T>>,
    unread: usize,
    index: usize,
    _marker: std::marker::PhantomData<&'a T>,
}

impl<'a, T: Event> Iterator for EventIter<'a, T> {
    type Item = EventRef<T>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.unread > 0 {
            self.unread -= 1;
            let item = EventRef {
                events: self.events.read_arc(),
                index: self.index,
            };
            self.index += 1;
            Some(item)
        } else {
            None
        }
    }
}

impl<T: Event> SystemParam for EventTx<T> {
    fn access() -> SystemAccess {
        SystemAccess {
            exclusive: false,
            resources_read: Vec::new(),
            resources_written: vec![TypeId::of::<Events<T>>()],
            components_read: Vec::new(),
            components_written: Vec::new(),
        }
    }

    fn fetch(world: &World) -> Option<Self>
    where
        Self: Sized,
    {
        world.get_resource_mut::<Events<T>>().map(EventTx::new)
    }
}

impl<T: Event> SystemParam for EventRx<T> {
    fn access() -> SystemAccess {
        SystemAccess {
            exclusive: false,
            resources_read: Vec::new(),
            resources_written: vec![TypeId::of::<Events<T>>()],
            components_read: Vec::new(),
            components_written: Vec::new(),
        }
    }

    fn fetch(world: &World) -> Option<Self>
    where
        Self: Sized,
    {
        world.get_resource::<Events<T>>().map(EventRx::new)
    }
}

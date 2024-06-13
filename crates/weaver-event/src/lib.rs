use std::collections::{vec_deque, VecDeque};

use weaver_ecs::{
    component::{Res, ResMut},
    prelude::Resource,
};

pub mod prelude {
    pub use super::{Event, EventRx, EventTx};
}

pub trait Event: 'static + Send + Sync {}

#[derive(Resource)]
pub struct Events<T: Event> {
    events: VecDeque<T>,
}

impl<T: Event> Default for Events<T> {
    fn default() -> Self {
        Self {
            events: VecDeque::new(),
        }
    }
}

impl<T: Event> Events<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.events.clear();
    }

    pub fn drain(&mut self) -> impl Iterator<Item = T> + '_ {
        self.events.drain(..)
    }

    pub fn send(&mut self, event: T) {
        self.events.push_back(event);
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
        self.events.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.events.is_empty()
    }

    pub fn iter(&self) -> EventIter<'_, T> {
        EventIter {
            iter: self.events.events.iter(),
            unread: self.events.events.len(),
        }
    }
}

pub struct EventIter<'a, T: Event> {
    iter: vec_deque::Iter<'a, T>,
    unread: usize,
}

impl<'a, T: Event> Iterator for EventIter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.unread > 0 {
            self.unread -= 1;
            self.iter.next()
        } else {
            None
        }
    }
}

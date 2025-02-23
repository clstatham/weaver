use std::{
    any::TypeId,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    task::Poll,
};

use futures_util::{FutureExt, Stream, StreamExt, pin_mut};
use weaver_ecs::{
    change_detection::Tick,
    system::{SystemAccess, SystemParam},
    world::World,
};
use weaver_util::prelude::*;

pub mod prelude {
    pub use super::{Event, EventRx, EventTx};
    pub use futures_util::{Stream, StreamExt};
}

pub trait Event: 'static + Send + Sync + Clone {}
impl<T: 'static + Send + Sync + Clone> Event for T {}

#[derive(Clone)]
struct EventChannels<T: Event> {
    tx: async_broadcast::Sender<T>,
    rx: async_broadcast::Receiver<T>,
}

impl<T: Event> EventChannels<T> {
    pub fn new() -> Self {
        let (tx, rx) = async_broadcast::broadcast(1024);
        Self { tx, rx }
    }

    pub async fn send(&self, event: T) {
        self.tx.broadcast_direct(event).await.unwrap();
    }

    pub fn clear(&mut self) {
        while self.rx.recv().now_or_never().is_some() {}
    }

    pub async fn extend(&mut self, events: impl Stream<Item = T>) {
        pin_mut!(events);
        while let Some(Some(event)) = events.next().now_or_never() {
            self.send(event).await;
        }
    }
}

impl<T: Event> Stream for EventChannels<T> {
    type Item = T;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let poll = Pin::new(&mut self.rx).poll_recv(cx);
        match poll {
            Poll::Ready(Some(event)) => Poll::Ready(Some(event.unwrap())),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Ready(None),
        }
    }
}

pub struct Events<T: Event> {
    front_buffer: EventChannels<T>,
    back_buffer: EventChannels<T>,
    updated_tick: Arc<AtomicU64>,
}

impl<T: Event> Default for Events<T> {
    fn default() -> Self {
        Self {
            front_buffer: EventChannels::new(),
            back_buffer: EventChannels::new(),
            updated_tick: Arc::new(AtomicU64::new(0)),
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

    pub fn clear(&mut self) {
        self.front_buffer.clear();
        self.back_buffer.clear();
    }

    pub async fn update(&mut self, change_tick: Tick) {
        self.updated_tick
            .store(change_tick.as_u64(), Ordering::Relaxed);
        self.back_buffer.clear();
        self.back_buffer.extend(&mut self.front_buffer).await;
    }

    pub async fn send(&self, event: T) {
        self.front_buffer.send(event).await;
    }

    pub async fn send_default(&self)
    where
        T: Default,
    {
        self.send(T::default()).await;
    }
}

pub struct EventTx<T: Event> {
    events: Events<T>,
}

impl<T: Event> EventTx<T> {
    pub fn new(events: Events<T>) -> Self {
        Self { events }
    }

    pub async fn send(&mut self, event: T) {
        self.events.send(event).await;
    }
}

#[derive(Clone)]
pub struct EventRxState<T: Event> {
    front_buffer: async_broadcast::Receiver<T>,
    back_buffer: async_broadcast::Receiver<T>,
}

pub struct EventRx<T: Event> {
    state: EventRxState<T>,
    include_back_buffer: bool,
}

impl<T: Event> EventRx<T> {
    pub fn new(
        events: EventRxState<T>,
        last_run: Tick,
        this_run: Tick,
        event_updated_tick: u64,
    ) -> Self {
        // if the tick that the events were last updated on, older than the tick that the system is running on, then include the back buffer
        let tick = Tick::from_raw(event_updated_tick);
        let include_back_buffer = !tick.is_newer_than(last_run, this_run);
        Self {
            state: events,
            include_back_buffer,
        }
    }

    pub fn len(&self) -> usize {
        if self.include_back_buffer {
            self.state.front_buffer.len() + self.state.back_buffer.len()
        } else {
            self.state.front_buffer.len()
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<T: Event> Stream for EventRx<T> {
    type Item = T;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let front_buffer = Pin::new(&mut self.state.front_buffer).poll_next(cx);
        match front_buffer {
            Poll::Ready(Some(event)) => Poll::Ready(Some(event)),
            Poll::Ready(None) => {
                if self.include_back_buffer {
                    let back_buffer = Pin::new(&mut self.state.back_buffer).poll_next(cx);
                    match back_buffer {
                        Poll::Ready(Some(event)) => Poll::Ready(Some(event)),
                        _ => Poll::Ready(None),
                    }
                } else {
                    Poll::Ready(None)
                }
            }
            Poll::Pending => Poll::Ready(None),
        }
    }
}

impl<T: Event> SystemParam for EventTx<T> {
    type Item = EventTx<T>;
    type State = ();

    fn access() -> SystemAccess {
        SystemAccess {
            exclusive: true,
            resources_written: FxHashSet::from_iter([TypeId::of::<Events<T>>()]),
            ..Default::default()
        }
    }

    fn init_state(_world: &World) -> Self::State {}

    fn fetch(world: &World, _state: &()) -> Self::Item {
        if let Some(events) = world.get_resource::<Events<T>>() {
            EventTx::new(events.clone())
        } else {
            panic!("Events resource not found");
        }
    }
}

impl<T: Event> SystemParam for EventRx<T> {
    type Item = EventRx<T>;
    type State = EventRxState<T>;

    fn access() -> SystemAccess {
        SystemAccess {
            exclusive: false,
            resources_read: FxHashSet::from_iter([TypeId::of::<Events<T>>()]),
            ..Default::default()
        }
    }

    fn init_state(world: &World) -> Self::State {
        let events = world.get_resource::<Events<T>>().unwrap();
        let front_buffer = events.front_buffer.rx.clone();
        let back_buffer = events.back_buffer.rx.clone();
        EventRxState {
            front_buffer,
            back_buffer,
        }
    }

    fn fetch(world: &World, state: &Self::State) -> Self::Item {
        if let Some(events) = world.get_resource::<Events<T>>() {
            EventRx::new(
                state.clone(),
                world.last_change_tick(),
                world.read_change_tick(),
                events.updated_tick.load(Ordering::Relaxed),
            )
        } else {
            panic!("Events resource not found");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::stream::StreamExt;
    use weaver_ecs::world::World;

    #[derive(Clone, Debug, Default)]
    struct TestEvent {
        value: u32,
    }

    #[tokio::test]
    async fn test_event() {
        let world = World::default();
        world.insert_resource(Events::<TestEvent>::new());

        let mut tx = EventTx::<TestEvent>::fetch(&world, &());

        tx.send(TestEvent { value: 1 }).await;
        tx.send(TestEvent { value: 2 }).await;
        tx.send(TestEvent { value: 3 }).await;

        let events = world.get_resource::<Events<TestEvent>>().unwrap();
        assert_eq!(events.front_buffer.rx.len(), 3);

        let rx_state = EventRx::<TestEvent>::init_state(&world);
        let mut rx = EventRx::<TestEvent>::fetch(&world, &rx_state);
        assert_eq!(rx.len(), 3);

        let event = rx.next().await.unwrap();
        assert_eq!(event.value, 1);

        let event = rx.next().await.unwrap();
        assert_eq!(event.value, 2);

        let event = rx.next().await.unwrap();
        assert_eq!(event.value, 3);

        assert!(rx.next().await.is_none());
    }
}

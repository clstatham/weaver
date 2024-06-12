use std::time::Duration;

use crossbeam_channel::{Receiver, Sender, TryRecvError};
use weaver_ecs::prelude::Resource;
use weaver_util::prelude::{anyhow, Result};

pub mod prelude {
    pub use super::{Event, EventRx, EventTx};
}

pub trait Event: 'static + Send + Sync {}

#[derive(Resource)]
pub struct EventRx<T: Event> {
    pub(crate) rx: Receiver<T>,
}

impl<T: Event> Clone for EventRx<T> {
    fn clone(&self) -> Self {
        Self {
            rx: self.rx.clone(),
        }
    }
}

impl<T: Event> EventRx<T> {
    pub fn try_recv(&mut self) -> Result<Option<T>> {
        match self.rx.try_recv() {
            Ok(message) => Ok(Some(message)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err(anyhow!("Disconnected")),
        }
    }

    pub fn try_iter(&mut self) -> impl Iterator<Item = T> + '_ {
        self.rx.try_iter()
    }

    pub fn recv_blocking(&self) -> Result<T> {
        self.rx.recv().map_err(|_| anyhow!("Disconnected"))
    }

    pub fn recv_timeout(&self, timeout: Duration) -> Result<Option<T>> {
        match self.rx.recv_timeout(timeout) {
            Ok(message) => Ok(Some(message)),
            Err(_) => Ok(None),
        }
    }

    pub fn recv_all(&self) -> Vec<T> {
        self.rx.try_iter().collect()
    }
}

#[derive(Resource)]
pub struct EventTx<T: Event> {
    pub(crate) tx: Sender<T>,
}

impl<T: Event> Clone for EventTx<T> {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
        }
    }
}

impl<T: Event> EventTx<T> {
    pub fn send(&self, message: T) -> Result<()> {
        self.tx.send(message).map_err(|_| anyhow!("Disconnected"))
    }
}

#[derive(Resource, Clone)]
pub struct EventChannel<T: Event> {
    pub tx: EventTx<T>,
    pub rx: EventRx<T>,
}

impl<T: Event> Default for EventChannel<T> {
    fn default() -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        Self {
            tx: EventTx { tx },
            rx: EventRx { rx },
        }
    }
}

impl<T: Event> EventChannel<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn tx(&self) -> EventTx<T> {
        self.tx.clone()
    }

    pub fn rx(&self) -> EventRx<T> {
        self.rx.clone()
    }

    pub fn send(&self, message: T) -> Result<()> {
        self.tx.send(message)
    }

    pub fn try_recv(&mut self) -> Result<Option<T>> {
        self.rx.try_recv()
    }

    pub fn try_iter(&mut self) -> impl Iterator<Item = T> + '_ {
        self.rx.try_iter()
    }

    pub fn recv_blocking(&self) -> Result<T> {
        self.rx.recv_blocking()
    }

    pub fn recv_timeout(&self, timeout: Duration) -> Result<Option<T>> {
        self.rx.recv_timeout(timeout)
    }

    pub fn recv_all(&self) -> Vec<T> {
        self.rx.recv_all()
    }
}

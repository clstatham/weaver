use std::time::Duration;

use crossbeam_channel::{Receiver, Sender, TryRecvError};
use weaver_ecs::prelude::Component;
use weaver_util::prelude::{anyhow, Result};

pub trait Event: 'static + Sized + Send + Sync {
    type Message: 'static + Send + Sync;

    fn new_tx_rx() -> (EventTx<Self>, EventRx<Self>);
}

#[derive(Component)]
pub struct EventRx<T: Event> {
    pub(crate) rx: Receiver<T::Message>,
}

impl<T: Event> EventRx<T> {
    pub fn try_recv(&mut self) -> Result<Option<T::Message>> {
        match self.rx.try_recv() {
            Ok(message) => Ok(Some(message)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err(anyhow!("Disconnected")),
        }
    }

    pub fn try_iter(&mut self) -> impl Iterator<Item = T::Message> + '_ {
        self.rx.try_iter()
    }

    pub fn recv_blocking(&self) -> Result<T::Message> {
        self.rx.recv().map_err(|_| anyhow!("Disconnected"))
    }

    pub fn recv_timeout(&self, timeout: Duration) -> Result<Option<T::Message>> {
        match self.rx.recv_timeout(timeout) {
            Ok(message) => Ok(Some(message)),
            Err(_) => Ok(None),
        }
    }

    pub fn recv_all(&self) -> Vec<T::Message> {
        self.rx.try_iter().collect()
    }
}

#[derive(Component)]
pub struct EventTx<T: Event> {
    pub(crate) tx: Sender<T::Message>,
}

impl<T: Event> EventTx<T> {
    pub fn send(&self, message: T::Message) -> Result<()> {
        self.tx.send(message).map_err(|_| anyhow!("Disconnected"))
    }
}

use std::{
    pin::Pin,
    task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
};

fn noop_clone(_data: *const ()) -> RawWaker {
    noop_raw_waker()
}

fn noop(_data: *const ()) {}

const NOOP_WAKER_VTABLE: RawWakerVTable = RawWakerVTable::new(noop_clone, noop, noop, noop);

fn noop_raw_waker() -> RawWaker {
    RawWaker::new(std::ptr::null(), &NOOP_WAKER_VTABLE)
}

fn noop_waker() -> Waker {
    unsafe { Waker::from_raw(noop_raw_waker()) }
}

pub fn now_or_never<F: Future>(mut future: F) -> Option<F::Output> {
    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);
    let future = unsafe { Pin::new_unchecked(&mut future) };
    let poll = future.poll(&mut cx);
    match poll {
        Poll::Ready(output) => Some(output),
        Poll::Pending => None,
    }
}

pub fn check_ready<F: Future + Unpin>(future: &mut F) -> Option<F::Output> {
    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);
    let future = Pin::new(future);
    let poll = future.poll(&mut cx);
    match poll {
        Poll::Ready(output) => Some(output),
        Poll::Pending => None,
    }
}

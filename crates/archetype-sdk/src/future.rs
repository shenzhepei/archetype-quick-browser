use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll, Waker},
    thread,
};

struct State<T> {
    result: Option<T>,
    waker: Option<Waker>,
}

/// A Runtime operation that can be awaited without selecting an async executor.
pub struct SdkFuture<T> {
    state: Arc<Mutex<State<T>>>,
}

impl<T> SdkFuture<T> {
    pub(crate) fn ready(result: T) -> Self {
        Self {
            state: Arc::new(Mutex::new(State {
                result: Some(result),
                waker: None,
            })),
        }
    }

    pub(crate) fn spawn(
        name: &str,
        worker: impl FnOnce() -> T + Send + 'static,
        spawn_failure: impl FnOnce(std::io::Error) -> T,
    ) -> Self
    where
        T: Send + 'static,
    {
        let state = Arc::new(Mutex::new(State {
            result: None,
            waker: None,
        }));
        let worker_state = Arc::clone(&state);
        let spawn = thread::Builder::new().name(name.to_owned()).spawn(move || {
            let result = worker();
            let waker = worker_state.lock().ok().and_then(|mut state| {
                state.result = Some(result);
                state.waker.take()
            });
            if let Some(waker) = waker {
                waker.wake();
            }
        });
        if let Err(error) = spawn
            && let Ok(mut state) = state.lock()
        {
            state.result = Some(spawn_failure(error));
        }
        Self { state }
    }
}

impl<T> Future for SdkFuture<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        let Ok(mut state) = self.state.lock() else {
            return Poll::Pending;
        };
        if let Some(result) = state.result.take() {
            Poll::Ready(result)
        } else {
            if state
                .waker
                .as_ref()
                .is_none_or(|waker| !waker.will_wake(context.waker()))
            {
                state.waker = Some(context.waker().clone());
            }
            Poll::Pending
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use super::*;

    #[test]
    fn worker_future_is_pending_before_completion() {
        let (sender, receiver) = mpsc::sync_channel(1);
        let mut future = Box::pin(SdkFuture::spawn(
            "sdk-future-test",
            move || receiver.recv().unwrap(),
            |_| 0,
        ));
        let mut context = Context::from_waker(Waker::noop());

        assert!(future.as_mut().poll(&mut context).is_pending());
        sender.send(42).unwrap();
        loop {
            match future.as_mut().poll(&mut context) {
                Poll::Ready(value) => {
                    assert_eq!(value, 42);
                    break;
                }
                Poll::Pending => thread::yield_now(),
            }
        }
    }
}

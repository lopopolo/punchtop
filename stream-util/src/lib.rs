// stream-util is based on stream-cancel@0.4.4
// <https://github.com/jonhoo/stream-cancel>
//
// MIT License
//
// Copyright (c) 2016 Jon Gjengset

use futures::future::Shared;
use futures::prelude::*;
use futures::sync::mpsc::UnboundedReceiver;
use futures::sync::oneshot;

#[derive(Debug)]
pub struct Trigger(Option<oneshot::Sender<()>>);

impl Trigger {
    pub fn terminate(self) {
        drop(self);
    }
}

impl Drop for Trigger {
    fn drop(&mut self) {
        if let Some(trigger) = self.0.take() {
            let _ = trigger.send(());
        }
    }
}

#[derive(Clone, Debug)]
pub struct Valve(Shared<oneshot::Receiver<()>>);

impl Future for Valve {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Result<Async<Self::Item>, Self::Error> {
        match self.0.poll() {
            Ok(Async::Ready(_)) => Ok(Async::Ready(())),
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Err(_) => Err(()),
        }
    }
}

pub fn valve() -> (Trigger, Valve) {
    let (trigger, valve) = oneshot::channel();
    (Trigger(Some(trigger)), Valve(valve.shared()))
}

#[derive(Debug, Eq, PartialEq)]
enum DrainState {
    Active,
    Draining,
}

#[derive(Debug)]
pub struct Drain<S, F> {
    receiver: S,
    until: F,
    state: DrainState,
}

impl<S, F> Stream for Drain<UnboundedReceiver<S>, F>
where
    F: Future<Item = (), Error = ()>,
{
    type Item = S;
    type Error = ();

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        if self.state == DrainState::Active {
            if let Ok(Async::Ready(_)) = self.until.poll() {
                // Drain trigger has resolved, close the underlying stream to
                // start a graceful drain and return a result indicating the
                // stream is terminated.
                self.receiver.close();
                self.state = DrainState::Draining;
            }
        }
        self.receiver.poll()
    }
}

pub trait Drainable: Stream {
    fn drain<F>(self, trigger: F) -> Drain<Self, F::Future>
    where
        F: IntoFuture<Item = (), Error = ()>,
        Self: Sized,
    {
        Drain {
            receiver: self,
            until: trigger.into_future(),
            state: DrainState::Active,
        }
    }
}

impl<S> Drainable for S where S: Stream {}

#[derive(Debug)]
pub struct Cancel<S, F> {
    stream: S,
    until: F,
}

impl<S, F> Stream for Cancel<S, F>
where
    S: Stream,
    F: Future<Item = (), Error = ()>,
{
    type Item = S::Item;
    type Error = S::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        if let Ok(Async::Ready(_)) = self.until.poll() {
            // Cancel trigger has resolved, short-circuit the underlying stream
            // and return a result indicating the stream is terminated.
            return Ok(Async::Ready(None));
        }
        self.stream.poll()
    }
}

pub trait Cancelable: Stream {
    fn cancel<F>(self, trigger: F) -> Cancel<Self, F::Future>
    where
        F: IntoFuture<Item = (), Error = ()>,
        Self: Sized,
    {
        Cancel {
            stream: self,
            until: trigger.into_future(),
        }
    }
}

impl<S> Cancelable for S where S: Stream {}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::sync::mpsc;
    use futures::Future;

    #[test]
    fn terminate_drains_receiver() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;
        use std::thread;

        let (trigger, valve) = valve();
        let (sender, receiver) = mpsc::unbounded::<()>();

        let counter = Arc::new(AtomicUsize::new(0));
        let msg_counter = counter.clone();
        sender.unbounded_send(()).unwrap();
        sender.unbounded_send(()).unwrap();

        // Trigger the drain before the channel starts consuming messages.
        // Expect all existing messages to be drained from the channel.
        trigger.terminate();
        let chan = thread::spawn(move || {
            let task = receiver
                .drain(valve)
                .for_each(move |_| {
                    msg_counter.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                })
                .map_err(|e| eprintln!("receive failed: {:?}", e));
            // start send-receive channel
            tokio::run(task);
        });

        // The receiver thread will normally never exit, since the sender is
        // open. With a `Drain` we can close the receiver and drain any messages
        // still in the channel.
        chan.join().unwrap();
        assert_eq!(2_usize, counter.load(Ordering::SeqCst));
    }

    #[test]
    fn drop_drains_receiver() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;
        use std::thread;

        let valve = {
            // Drop the trigger by letting it fall out of scope.
            let (_trigger, valve) = valve();
            valve
        };
        let (sender, receiver) = mpsc::unbounded::<()>();

        let counter = Arc::new(AtomicUsize::new(0));
        let msg_counter = counter.clone();
        sender.unbounded_send(()).unwrap();
        sender.unbounded_send(()).unwrap();

        let chan = thread::spawn(move || {
            let task = receiver
                .drain(valve)
                .for_each(move |_| {
                    msg_counter.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                })
                .map_err(|e| eprintln!("receive failed: {:?}", e));
            // start send-receive channel
            tokio::run(task);
        });

        // The receiver thread will normally never exit, since the sender is
        // open. With a `Drain` we can close the receiver and drain any messages
        // still in the channel.
        chan.join().unwrap();
        assert_eq!(2_usize, counter.load(Ordering::SeqCst));
    }

    #[test]
    fn terminate_cancels_stream() {
        use std::thread;
        use std::time::Duration;
        use tokio::timer::Interval;

        let (trigger, valve) = valve();
        let interval = thread::spawn(move || {
            let task = Interval::new_interval(Duration::from_millis(250))
                .cancel(valve)
                .for_each(|_| Ok(()))
                .map_err(|e| eprintln!("interval failed: {:?}", e));
            // start send-receive channel
            tokio::run(task);
        });

        // The interval thread will normally never exit, since the interval is
        // repeats forever. With a `Cancel` we can short circuit the stream.
        trigger.terminate();
        interval.join().unwrap();
    }

    #[test]
    fn drop_cancels_stream() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;
        use std::thread;
        use std::time::Duration;
        use tokio::timer::Interval;

        let counter = Arc::new(AtomicUsize::new(0));
        let msg_counter = counter.clone();

        let valve = {
            // Drop the trigger by letting it fall out of scope.
            let (_trigger, valve) = valve();
            valve
        };
        let interval = thread::spawn(move || {
            let task = Interval::new_interval(Duration::from_millis(250))
                .cancel(valve)
                .for_each(move |_| {
                    msg_counter.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                })
                .map_err(|e| eprintln!("interval failed: {:?}", e));
            // start send-receive channel
            tokio::run(task);
        });

        // The interval thread will normally never exit, since the interval is
        // repeats forever. With a `Cancel` we can short circuit the stream.
        interval.join().unwrap();
        assert_eq!(0_usize, counter.load(Ordering::SeqCst));
    }

    #[test]
    fn cancel_does_not_drain_receiver() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;
        use std::thread;

        let (trigger, valve) = valve();
        let (sender, receiver) = mpsc::unbounded::<()>();

        let counter = Arc::new(AtomicUsize::new(0));
        let msg_counter = counter.clone();
        sender.unbounded_send(()).unwrap();
        sender.unbounded_send(()).unwrap();

        // Trigger the cancel before the channel starts consuming messages.
        // Expect no existing messages to be drained from the channel.
        trigger.terminate();
        let chan = thread::spawn(move || {
            let task = receiver
                .cancel(valve)
                .for_each(move |_| {
                    msg_counter.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                })
                .map_err(|e| eprintln!("receive failed: {:?}", e));
            // start send-receive channel
            tokio::run(task);
        });

        chan.join().unwrap();
        assert_eq!(0_usize, counter.load(Ordering::SeqCst));
    }
}

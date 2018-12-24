#![deny(missing_docs, warnings)]
#![doc(test(attr(deny(warnings))))]
// stream-util is based on stream-cancel@0.4.4
// <https://github.com/jonhoo/stream-cancel>
//
// MIT License
//
// Copyright (c) 2016 Jon Gjengset

//! Crate `stream-util` provides mechanisms for canceling a
//! [`Stream`](futures::stream::Stream) and draining a
//! [`Receiver`](futures::sync::mpsc::Receiver) or
//! [`UnboundedReceiver`](futures::sync::mpsc::UnboundedReceiver).
//!
//! # Usage
//!
//! To use this crate, add `stream-util` as a dependency to your project's
//! `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! stream-util = { git = "https://github.com/lopopolo/punchtop" }
//! ```
//!
//! # Drain
//!
//! The extension trait [`Drainable`] provides a new
//! [`Receiver`](futures::sync::mpsc::Receiver) and
//! [`UnboundedReceiver`](futures::sync::mpsc::UnboundedReceiver) combinator,
//! [`drain`](Drainable::drain). [`Drain`] yields elements from the underlying
//! channel until the provided [`Future`](futures::future::Future) resolves. It
//! then closes the receiver and continues to yield the remaining elements in
//! the channel until it is empty.
//!
//! ## Example: Drain a Channel
//!
//! The following code creates an
//! [`mpsc::unbounded`](futures::sync::mpsc::unbounded) channel and drains two
//! messages from the channel after it has been canceled.
//!
//! ```rust
//! use std::thread;
//! use futures::{Future, Stream};
//! use stream_util::{valve, Drainable};
//! use futures::sync::mpsc;
//!
//! let (trigger, valve) = valve();
//! let (sender, receiver) = mpsc::unbounded::<()>();
//!
//! sender.unbounded_send(()).unwrap();
//! sender.unbounded_send(()).unwrap();
//!
//! // Trigger the drain before the channel starts consuming messages. Expect all
//! // existing messages to be drained from the channel.
//! trigger.terminate();
//! let chan = thread::spawn(move || {
//!     let task = receiver
//!         .drain(valve)
//!         .for_each(|_| Ok(()))
//!         .map_err(|e| eprintln!("receive failed: {:?}", e));
//!     // start send-receive channel
//!     tokio::run(task);
//! });
//!
//! // The receiver thread will normally never exit, since the sender is open. With a
//! // `Drain` we can close the receiver and drain any messages still in the channel.
//! chan.join().unwrap();
//! ```
//!
//! # Cancel
//!
//! The extension trait [`Cancelable`] provides a new
//! [`Stream`](futures::stream::Stream) combinator,
//! [`cancel`](Cancelable::cancel). [`Cancel`] yields elements from the
//! underlying [`Stream`](futures::stream::Stream) until the provided
//! [`Future`](futures::future::Future) resolves. It then short-circuits the
//! underlying stream by returning `Async::Ready(None)`, which stops polling of
//! the underlying [`Stream`](futures::stream::Stream).
//!
//! ## Example: Cancel an Interval
//!
//! The following code creates an infinite stream from a `tokio`
//! [`Interval`](https://docs.rs/tokio/0.1/tokio/timer/struct.Interval.html)
//! and cancels it.
//!
//! ```rust
//! use std::thread;
//! use std::time::Duration;
//! use futures::{Future, Stream};
//! use stream_util::{valve, Cancelable};
//! use tokio::timer::Interval;
//!
//! let (trigger, valve) = valve();
//! let interval = thread::spawn(move || {
//!     let task = Interval::new_interval(Duration::from_millis(250))
//!         .cancel(valve)
//!         .for_each(|_| Ok(()))
//!         .map_err(|e| eprintln!("interval failed: {:?}", e));
//!     // start send-receive channel
//!     tokio::run(task);
//! });
//!
//! // The interval thread will normally never exit, since the interval repeats
//! // forever. With a `Cancel` we can short circuit the stream.
//! trigger.terminate();
//! interval.join().unwrap();
//! ```
//!
//! # Trigger and Valve
//!
//! The [`valve`] function returns a tuple of ([`Trigger`], [`Valve`]) as a
//! convenience for generating a [`Future`](futures::future::Future) for the
//! [`drain`](Drainable::drain) and [`cancel`](Cancelable::cancel) combinators
//! that resolves when triggered.

use futures::future::Shared;
use futures::prelude::*;
use futures::sync::mpsc::{Receiver, UnboundedReceiver};
use futures::sync::oneshot;

/// A remote trigger for canceling or draining a
/// [`Stream`](futures::stream::Stream) with a [`Valve`].
///
/// `Trigger` implements [`Drop`](std::ops::Drop) and will trigger when it goes
/// out of scope.
#[derive(Debug)]
pub struct Trigger(Option<oneshot::Sender<()>>);

impl Trigger {
    /// Consume the `Trigger` and resolve the linked [`Valve`].
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

/// Cancel or drain a [`Stream`](futures::stream::Stream) when resolved by a
/// [`Trigger`].
///
/// `Valve` implements a unit [`Future`](futures::future::Future) enabling it
/// to be used with the [`drain`](Drainable::drain) and
/// [`cancel`](Cancelable::cancel) combinators.
///
/// `Valve` is cloneable and may be used with multiple
/// [`Stream`](futures::stream::Stream)s.
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

/// Create a matching [`Trigger`] and [`Valve`].
pub fn valve() -> (Trigger, Valve) {
    let (trigger, valve) = oneshot::channel();
    (Trigger(Some(trigger)), Valve(valve.shared()))
}

#[derive(Debug, Eq, PartialEq)]
enum DrainState {
    Active,
    Draining,
}

/// Wrapper around [`Receiver`](futures::sync::mpsc::Receiver) and
/// [`UnboundedReceiver`](futures::sync::mpsc::UnboundedReceiver) that enables
/// the receiver to be canceled and fully drained by closing it safely.
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

impl<S, F> Stream for Drain<Receiver<S>, F>
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

/// Extension trait that exposes the [`drain`](Drainable::drain) method for
/// [`Receiver`](futures::sync::mpsc::Receiver) and
/// [`UnboundedReceiver`](futures::sync::mpsc::UnboundedReceiver).
pub trait Drainable: Stream {
    /// Create a new [`Stream`](futures::stream::Stream) that closes and drains
    /// when `trigger` resolves.
    ///
    /// The `Stream` can be polled until all outstanding messages are drained.
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

impl<S> Drainable for Receiver<S> {}
impl<S> Drainable for UnboundedReceiver<S> {}

/// Wrapper around [`Stream`](futures::stream::Stream) that enables the stream
/// to be canceled and terminated.
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

/// Extension trait that exposes the [`cancel`](Cancelable::cancel) method for
/// [`Stream`](futures::stream::Stream).
pub trait Cancelable: Stream {
    /// Create a new [`Stream`](futures::stream::Stream) that yields the items
    /// from the receiver until `trigger` resolves.
    ///
    /// When `trigger` resolves, short circuit the stream by returning
    /// `Async::Ready(None)`.
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
    fn terminate_drains_bounded_receiver() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;
        use std::thread;

        let (trigger, valve) = valve();
        let (mut sender, receiver) = mpsc::channel::<()>(1);

        let counter = Arc::new(AtomicUsize::new(0));
        let msg_counter = counter.clone();
        sender.try_send(()).unwrap();
        sender.try_send(()).unwrap();
        assert!(sender.try_send(()).is_err());

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

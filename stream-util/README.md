# stream-util

Crate `stream-util` provides mechanisms for draining a [`Receiver`] or
[`UnboundedReceiver`] and canceling a [`Stream`].

## Usage

To use this crate, add `stream-util` as a dependency to your project's
`Cargo.toml`:

```toml
[dependencies]
stream-util = { git = "https://github.com/lopopolo/punchtop" }
```

## Drain

The extension trait `Drainable` provides a new [`Receiver`] and
[`UnboundedReceiver`] combinator, `drain`. `Drain` yields elements from the
underlying channel until the provided [`Future`] resolves. It then closes the
receiver and continues to yield the remaining elements in the channel until it
is empty.

### Example: Drain a Channel

The following code creates an [`mpsc::unbounded`] channel and drains two
messages from the channel after it has been canceled.

```rust
use futures::sync::mpsc;
use futures::{Future, Stream};
use std::thread;
use stream_util::{self, Drainable};

let (trigger, valve) = stream_util::valve();
let (sender, receiver) = mpsc::unbounded::<()>();

sender.unbounded_send(()).unwrap();
sender.unbounded_send(()).unwrap();

// Trigger the drain before the channel starts consuming messages. Expect all
// existing messages to be drained from the channel.
trigger.terminate();
let chan = thread::spawn(move || {
    let task = receiver
        .drain(valve)
        .for_each(|_| Ok(()))
        .map_err(|e| eprintln!("receive failed: {:?}", e));
    // start send-receive channel
    tokio::run(task);
});

// The receiver thread will normally never exit, since the sender is open. With a
// `Drain` we can close the receiver and drain any messages still in the channel.
chan.join().unwrap();
```

## Cancel

The extension trait `Cancelable` provides a new [`Stream`] combinator, `cancel`.
`Cancel` yields elements from the underlying [`Stream`] until the provided
[`Future`] resolves. It then short-circuits the underlying stream by returning
`Async::Ready(None)`, which stops polling of the underlying [`Stream`].

### Example: Cancel an Interval

The following code creates an infinite stream from a `tokio` [`Interval`] and
cancels it.

```rust
use futures::{Future, Stream};
use std::thread;
use std::time::Duration;
use stream_util::{self, Cancelable};
use tokio::timer::Interval;

let (trigger, valve) = stream_util::valve();
let interval = thread::spawn(move || {
    let task = Interval::new_interval(Duration::from_millis(250))
        .cancel(valve)
        .for_each(|_| Ok(()))
        .map_err(|e| eprintln!("interval failed: {:?}", e));
    // start send-receive channel
    tokio::run(task);
});

// The interval thread will normally never exit, since the interval repeats
// forever. With a `Cancel` we can short circuit the stream.
trigger.terminate();
interval.join().unwrap();
```

## Trigger and Valve

The `valve` function returns a tuple of (`Trigger`, `Valve`) as a convenience
for generating a [`Future`] for the `drain` and `cancel` combinators that
resolves when triggered.

## License

`stream-util` is licensed under the MIT license.

`stream-util` is based on
[`stream-cancel`](https://github.com/jonhoo/stream-cancel) by Jon Gjengset.
`stream-cancel` is dual-licensed under the MIT and Apache 2.0 licenses.

[`interval`]: https://docs.rs/tokio/0.1/tokio/timer/struct.Interval.html
[`future`]: https://docs.rs/futures/0.1/futures/future/trait.Future.html
[`receiver`]: https://docs.rs/futures/0.1/futures/sync/mpsc/struct.Receiver.html
[`stream`]: https://docs.rs/futures/0.1/futures/stream/trait.Stream.html
[`unboundedreceiver`]:
  https://docs.rs/futures/0.1/futures/sync/mpsc/struct.UnboundedReceiver.html
[`mpsc::unbounded`]:
  https://docs.rs/futures/0.1/futures/sync/mpsc/fn.unbounded.html

use futures::prelude::*;
use futures::sync::mpsc::UnboundedReceiver;
use futures::sync::oneshot;

pub type DrainListener = oneshot::Receiver<()>;
pub type DrainTrigger = oneshot::Sender<()>;

#[derive(Debug, Eq, PartialEq)]
enum DrainState {
    Active,
    Draining,
}

pub fn drain<F, S>(receiver: UnboundedReceiver<S>, trigger: F) -> Drain<S, F>
where
    F: Future<Item = (), Error = ()>,
{
    Drain {
        receiver,
        until: trigger,
        state: DrainState::Active,
    }
}

#[derive(Debug)]
pub struct Drain<S, F> {
    receiver: UnboundedReceiver<S>,
    until: F,
    state: DrainState,
}

impl<S, F> Stream for Drain<S, F>
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

#[cfg(test)]
mod tests {
    use super::*;
    use futures::sync::{mpsc, oneshot};
    use futures::Stream;

    #[test]
    fn terminates_stream() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;
        use std::thread;

        let (trigger, shutdown) = oneshot::channel::<()>();
        let (sender, receiver) = mpsc::unbounded::<()>();

        let counter = Arc::new(AtomicUsize::new(0));
        let msg_counter = counter.clone();
        sender.unbounded_send(()).unwrap();
        let chan = thread::spawn(move || {
            let task = drain(receiver, shutdown.map(|_| ()).map_err(|_| ()))
                .for_each(move |_| {
                    msg_counter.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                })
                .map_err(|e| eprintln!("receive failed: {:?}", e));
            // start send-receive channel
            tokio::run(task);
        });
        sender.unbounded_send(()).unwrap();

        // The receiver thread will normally never exit, since the sender is
        // open. With a `Drain` we can close the receiver and drain any messages
        // still in the channel.
        trigger.send(()).unwrap();
        chan.join().unwrap();
        assert_eq!(2usize, counter.load(Ordering::SeqCst));
    }

    #[test]
    fn drains_messages_in_channel() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;
        use std::thread;

        let (trigger, shutdown) = oneshot::channel::<()>();
        let (sender, receiver) = mpsc::unbounded::<()>();

        let counter = Arc::new(AtomicUsize::new(0));
        let msg_counter = counter.clone();
        sender.unbounded_send(()).unwrap();
        sender.unbounded_send(()).unwrap();

        // Drain the channel before it starts consuming messages. Expect all
        // existing messages to be drained from the channel.
        trigger.send(()).unwrap();
        let chan = thread::spawn(move || {
            let task = drain(receiver, shutdown.map(|_| ()).map_err(|_| ()))
                .for_each(move |_| {
                    msg_counter.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                })
                .map_err(|e| eprintln!("receive failed: {:?}", e));
            // start send-receive channel
            tokio::run(task);
        });

        chan.join().unwrap();
        assert_eq!(2usize, counter.load(Ordering::SeqCst));
    }
}

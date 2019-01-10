use std::io;
use std::time::Duration;

use futures::prelude::*;
use futures::sync::mpsc::UnboundedSender;
use futures::Future;
use futures_locks::RwLock;
use stream_util::{Cancelable, Valve};
use tokio_timer::Interval;

use crate::channel::Responder;
use crate::proto::CastMessage;
use crate::{Command, ConnectState, Status};

pub fn keepalive(
    valve: Valve,
    command: UnboundedSender<Command>,
) -> impl Future<Item = (), Error = ()> {
    Interval::new_interval(Duration::new(5, 0))
        .cancel(valve)
        .map(|_| Command::Ping)
        .or_else(|err| {
            warn!("Error on heartbeat interval: {:?}", err);
            // Attempt to recover from errors on the heartbeat channel
            Ok(Command::Ping) as Result<Command, ()>
        })
        .forward(command.sink_map_err(|err| warn!("Error on sink heartbeat: {:?}", err)))
        .map(|_| ())
        .or_else(|err| {
            warn!("Error on heartbeat: {:?}", err);
            // Attempt to recover from errors on the heartbeat channel
            Ok(())
        })
}

pub fn poll_status(
    valve: Valve,
    state: RwLock<ConnectState>,
    tx: UnboundedSender<Command>,
) -> impl Future<Item = (), Error = ()> {
    Interval::new_interval(Duration::from_millis(150))
        .cancel(valve)
        .map_err(|err| warn!("Error on status interval: {:?}", err))
        .and_then(move |_| {
            let tx = tx.clone();
            let status = state.clone().with_read(move |state| {
                tx.unbounded_send(Command::ReceiverStatus).map_err(|_| ())?;
                if let Some(connect) = state.media_connection() {
                    tx.unbounded_send(Command::MediaStatus(connect.clone()))
                        .map_err(|_| ())?;
                }
                Ok(())
            });
            status.expect("lock spawn")
        })
        .for_each(|_| Ok(()))
}

pub fn respond(
    source: impl Stream<Item = CastMessage, Error = io::Error>,
    connect: &RwLock<ConnectState>,
    command: &UnboundedSender<Command>,
    status: &UnboundedSender<Status>,
) -> impl Future<Item = (), Error = ()> {
    let responder = Responder::new(connect, command, status);
    source
        .for_each(move |message| {
            if let Err(err) = responder.handle(&message) {
                warn!("responder handler error: {:?}", err);
                return Err(io::Error::new(io::ErrorKind::Other, err));
            }
            Ok(())
        })
        .map_err(|err| warn!("Error on responder: {:?}", err))
}

pub fn send(
    sink: impl Sink<SinkItem = Command, SinkError = io::Error>,
    command: impl Stream<Item = Command, Error = ()>,
) -> impl Future<Item = (), Error = ()> {
    command
        .forward(sink.sink_map_err(|err| warn!("Error on sink write: {:?}", err)))
        .map(|_| ())
        .or_else(|err| {
            warn!("Error on write: {:?}", err);
            // Attempt to recover from errors on the write channel
            Ok(())
        })
}

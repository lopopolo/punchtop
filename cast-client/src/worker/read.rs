use std::io;

use futures::prelude::*;
use futures::sync::mpsc::UnboundedSender;
use futures::Future;
use futures_locks::RwLock;

use crate::handler::Chain;
use crate::proto::CastMessage;
use crate::{Command, ConnectState, Status};

pub(crate) fn task(
    source: impl Stream<Item = CastMessage, Error = io::Error>,
    connect: RwLock<ConnectState>,
    command: UnboundedSender<Command>,
    status: UnboundedSender<Status>,
) -> impl Future<Item = (), Error = ()> {
    let handler = Chain::new(connect, command, status);
    source
        .for_each(move |message| {
            if let Err(err) = handler.handle(&message) {
                warn!("read handler error: {:?}", err);
                return Err(io::Error::new(io::ErrorKind::Other, err));
            }
            Ok(())
        })
        .map_err(|err| warn!("Error on read: {:?}", err))
}

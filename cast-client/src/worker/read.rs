use std::io;

use futures::prelude::*;
use futures::sync::mpsc::UnboundedSender;
use futures::Future;
use futures_locks::RwLock;

use crate::channel::Responder;
use crate::proto::CastMessage;
use crate::{Command, ConnectState, Status};

pub(crate) fn task(
    source: impl Stream<Item = CastMessage, Error = io::Error>,
    connect: RwLock<ConnectState>,
    command: UnboundedSender<Command>,
    status: UnboundedSender<Status>,
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

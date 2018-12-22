use std::io;

use futures::prelude::*;

use crate::Command;

pub(crate) fn task(
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

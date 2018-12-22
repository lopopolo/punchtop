use std::time::Duration;

use futures::prelude::*;
use futures::sync::mpsc::UnboundedSender;
use stream_util::{Cancelable, Valve};
use tokio_timer::Interval;

use crate::Command;

pub(crate) fn task(
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

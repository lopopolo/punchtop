use std::time::Duration;

use futures::prelude::*;
use futures::sync::mpsc::UnboundedSender;
use tokio_timer::Interval;

use crate::Command;

pub fn task(command: UnboundedSender<Command>) -> impl Future<Item = (), Error = ()> {
    Interval::new_interval(Duration::new(5, 0))
        .map(|_| Command::Ping)
        .map_err(|err| warn!("Error on heartbeat interval: {:?}", err))
        .forward(command.sink_map_err(|err| warn!("Error on sink heartbeat: {:?}", err)))
        .map(|_| ())
        .map_err(|err| warn!("Error on heartbeat: {:?}", err))
}

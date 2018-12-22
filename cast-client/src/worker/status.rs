use std::time::Duration;

use futures::prelude::*;
use futures::sync::mpsc::UnboundedSender;
use futures_locks::Mutex;
use stream_util::{Cancelable, Valve};
use tokio_timer::Interval;

use crate::{Command, ConnectState};

pub(crate) fn task(
    valve: Valve,
    state: Mutex<ConnectState>,
    tx: UnboundedSender<Command>,
) -> impl Future<Item = (), Error = ()> {
    Interval::new_interval(Duration::from_millis(150))
        .cancel(valve)
        .map_err(|err| warn!("Error on status interval: {:?}", err))
        .and_then(move |_| state.lock())
        .map_err(|err| warn!("Error on connect state lock: {:?}", err))
        .for_each(move |state| {
            let _ = tx.unbounded_send(Command::ReceiverStatus);
            if let Some(connect) = state.media_connection() {
                let _ = tx.unbounded_send(Command::MediaStatus(connect.clone()));
            }
            Ok(())
        })
        .map_err(|err| warn!("Error on status: {:?}", err))
}

use std::time::Duration;

use futures::prelude::*;
use futures::sync::mpsc::UnboundedSender;
use futures::Future;
use futures_locks::Mutex;
use tokio::timer::Interval;

use crate::cast::{Command, ConnectState, MediaConnection, SessionLifecycle};

pub fn task(
    state: Mutex<ConnectState>,
    tx: UnboundedSender<Command>,
) -> impl Future<Item = (), Error = ()> {
    Interval::new_interval(Duration::from_millis(150))
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

/// Register a media session id with the global connection state. Returns
/// `Some(state)` if the registration caused the media session id to change,
/// `None` otherwise.
pub fn register_media_session(
    state: &Mutex<ConnectState>,
    session: i64,
) -> impl Future<Item = Option<MediaConnection>, Error = ()> {
    state
        .lock()
        .map(move |mut state| {
            if state.set_media_session(Some(session)) {
                debug!("media session established: {}", session);
                state.lifecycle = SessionLifecycle::Established;
                state.media_connection()
            } else {
                None
            }
        })
        .map_err(|_| ())
}

/// Invalidate a media session id. This prevents the `task` from polling for
/// media status when the session is no longer valid (e.g. if a new load has
/// been schdeduled.
pub fn invalidate_media_connection(
    state: &Mutex<ConnectState>,
) -> impl Future<Item = (), Error = ()> {
    state
        .lock()
        .map(|mut state| {
            debug!("media session invalidated");
            state.lifecycle = SessionLifecycle::NoMediaSession;
        })
        .map_err(|_| ())
}

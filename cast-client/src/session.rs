use futures::prelude::*;
use futures_locks::RwLock;

use crate::{ConnectState, MediaConnection, SessionLifecycle};

/// Register a media session id with the global connection state. Returns
/// `Some(state)` if the registration caused the media session id to change,
/// `None` otherwise.
pub fn register(
    state: &RwLock<ConnectState>,
    session: i64,
) -> impl Future<Item = Option<MediaConnection>, Error = ()> {
    let register = state.with_write(move |mut state| {
        if state.set_media_session(Some(session)) {
            debug!("media session established: {}", session);
            state.lifecycle = SessionLifecycle::Established;
            Ok(state.media_connection())
        } else {
            Ok(None)
        }
    });
    register.expect("lock spawn")
}

/// Invalidate a media session id. This prevents the `status::task` from
/// polling for media status when the session is no longer valid (e.g. if a new
/// load has been schdeduled.
pub fn invalidate(state: &RwLock<ConnectState>) -> impl Future<Item = (), Error = ()> {
    let invalidate = state.with_write(|mut state| {
        debug!("media session invalidated");
        state.lifecycle = SessionLifecycle::NoMediaSession;
        Ok(())
    });
    invalidate.expect("lock spawn")
}

use futures::sync::mpsc::UnboundedSender;
use futures::Future;
use futures_locks::RwLock;

use crate::handler::{Error, Handler};
use crate::payload::media::Response;
use crate::session;
use crate::{Command, ConnectState, Status};

const NAMESPACE: &str = "urn:x-cast:com.google.cast.media";
const CHANNEL: &str = "media";

#[derive(Debug)]
pub struct Media {
    connect: RwLock<ConnectState>,
    command: UnboundedSender<Command>,
    status: UnboundedSender<Status>,
}

impl Media {
    pub fn new(
        connect: RwLock<ConnectState>,
        command: UnboundedSender<Command>,
        status: UnboundedSender<Status>,
    ) -> Self {
        Self {
            connect,
            command,
            status,
        }
    }
}

impl Handler for Media {
    type Payload = Response;

    fn channel(&self) -> &str {
        CHANNEL
    }

    fn namespace(&self) -> &str {
        NAMESPACE
    }

    fn handle(&self, payload: Self::Payload) -> Result<(), Error> {
        match payload {
            Response::MediaStatus { status, .. } => {
                let status = status.into_iter().next();
                let session = status.as_ref().map(|status| status.media_session_id);
                if let Some(session) = session {
                    let tx = self.status.clone();
                    let task = session::register(&self.connect, session);
                    let task = task.and_then(move |connect| {
                        if let Some(connect) = connect {
                            tx.unbounded_send(Status::MediaConnected(Box::new(connect)))
                                .map_err(|_| ())?;
                        }
                        Ok(())
                    });
                    tokio_executor::spawn(task);
                } else {
                    tokio_executor::spawn(session::invalidate(&self.connect));
                }
                if let (Some(state), false) = (status, self.status.is_closed()) {
                    self.status
                        .unbounded_send(Status::MediaState(Box::new(state)))
                        .map_err(|_| Error::StatusSend)?;
                }
                Ok(())
            }
            _ => Err(Error::UnknownPayload),
        }
    }
}

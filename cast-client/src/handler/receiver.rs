use futures::sync::mpsc::UnboundedSender;
use futures::Future;
use futures_locks::RwLock;

use crate::handler::{Error, Handler};
use crate::payload::receiver::Response;
use crate::{Command, ConnectState, Status, DEFAULT_MEDIA_RECEIVER_APP_ID};

const NAMESPACE: &str = "urn:x-cast:com.google.cast.receiver";
const CHANNEL: &str = "receiver";

#[derive(Debug)]
pub struct Receiver {
    connect: RwLock<ConnectState>,
    command: UnboundedSender<Command>,
    status: UnboundedSender<Status>,
}

impl Receiver {
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

impl Handler for Receiver {
    type Payload = Response;

    fn channel(&self) -> &str {
        CHANNEL
    }

    fn namespace(&self) -> &str {
        NAMESPACE
    }

    fn handle(&self, payload: Self::Payload) -> Result<(), Error> {
        let Response::ReceiverStatus { status, .. } = payload;
        let app = status
            .applications
            .iter()
            .find(|app| app.app_id == DEFAULT_MEDIA_RECEIVER_APP_ID);
        let session = app.map(|app| app.session_id.to_owned());
        let transport = app.map(|app| app.transport_id.to_owned());
        let status = self.status.clone();
        let command = self.command.clone();
        let connect = self.connect.write().and_then(move |mut state| {
            trace!("acquired connect state lock in receiver channel");
            if !state.set_session(session.deref()) || !state.set_transport(transport.deref()) {
                // Connection did not change
                return Ok(());
            }
            if let Some(ref connect) = state.receiver_connection() {
                debug!("connecting to transport {}", connect.transport);
                status
                    .unbounded_send(Status::Connected(Box::new(connect.clone())))
                    .map_err(|_| ())?;
                // we've connected to the default receiver. Now connect to the
                // transport backing the launched app session.
                command
                    .unbounded_send(Command::Connect(connect.clone()))
                    .map_err(|_| ())?;
            }
            Ok(())
        });
        tokio_executor::spawn(connect);
        Ok(())
    }
}

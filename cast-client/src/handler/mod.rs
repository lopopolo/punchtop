use std::error;
use std::fmt;

use futures::sync::mpsc::UnboundedSender;
use futures_locks::RwLock;

use crate::proto::CastMessage;
use crate::{Command, ConnectState, Status};

pub mod connection;
pub mod heartbeat;
pub mod media;
pub mod receiver;

#[derive(Debug)]
pub enum Error {
    CommandSend,
    NamespaceMismatch,
    Parse,
    StatusSend,
    UnknownPayload,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl error::Error for Error {}

pub trait Handler {
    type Payload: serde::de::DeserializeOwned;

    fn namespace(&self) -> &str;

    fn channel(&self) -> &str;

    fn handle(&self, payload: Self::Payload) -> Result<(), Error>;

    fn try_handle(&self, message: &CastMessage) -> Result<(), Error> {
        if message.get_namespace() != self.namespace() {
            return Err(Error::NamespaceMismatch);
        }
        trace!("found message for {} channel", self.channel());
        let payload = serde_json::from_str::<Self::Payload>(message.get_payload_utf8())
            .map_err(|_| Error::Parse)?;
        self.handle(payload)
    }
}

#[derive(Debug)]
pub struct Chain {
    connection: connection::Connection,
    heartbeat: heartbeat::Heartbeat,
    media: media::Media,
    receiver: receiver::Receiver,
}

impl Chain {
    pub fn new(
        connect: RwLock<ConnectState>,
        command: UnboundedSender<Command>,
        status: UnboundedSender<Status>,
    ) -> Self {
        Chain {
            connection: connection::Connection,
            heartbeat: heartbeat::Heartbeat::new(command.clone()),
            media: media::Media::new(connect.clone(), command.clone(), status.clone()),
            receiver: receiver::Receiver::new(connect.clone(), command.clone(), status.clone()),
        }
    }

    pub fn handle(&self, message: &CastMessage) -> Result<(), Error> {
        // Try handlers in order of receive frequency
        match self.media.try_handle(message) {
            Err(Error::NamespaceMismatch) => {}
            response => return response,
        };
        match self.receiver.try_handle(message) {
            Err(Error::NamespaceMismatch) => {}
            response => return response,
        };
        match self.heartbeat.try_handle(message) {
            Err(Error::NamespaceMismatch) => {}
            response => return response,
        };
        match self.connection.try_handle(message) {
            Err(Error::NamespaceMismatch) => {}
            response => return response,
        };
        warn!("got message on unknown channel {}", message.get_namespace());
        Err(Error::UnknownPayload)
    }
}

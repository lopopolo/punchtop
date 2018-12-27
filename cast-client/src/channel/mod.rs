use std::error;
use std::fmt;

use futures::sync::mpsc::UnboundedSender;
use futures_locks::RwLock;
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::proto::{CastMessage, CastMessage_PayloadType, CastMessage_ProtocolVersion};
use crate::{Command, ConnectState, Status};

pub mod connection;
pub mod heartbeat;
pub mod media;
pub mod receiver;

pub const DEFAULT_DESTINATION_ID: &str = "receiver-0";
pub const DEFAULT_MEDIA_RECEIVER_APP_ID: &str = "CC1AD845";
pub const DEFAULT_SENDER_ID: &str = "sender-0";

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
    type Payload: DeserializeOwned;

    fn namespace(&self) -> &str;

    fn channel(&self) -> &str;

    fn handle(&self, payload: Self::Payload) -> Result<(), Error>;

    fn try_handle(&self, message: &CastMessage) -> Result<Option<()>, Error> {
        if message.get_namespace() != self.namespace() {
            return Ok(None);
        }
        trace!("found message for {} channel", self.channel());
        let payload = serde_json::from_str(message.get_payload_utf8()).map_err(|_| Error::Parse)?;
        self.handle(payload).map(Some)
    }
}

#[derive(Debug)]
pub struct Responder {
    connection: connection::Handler,
    heartbeat: heartbeat::Handler,
    media: media::Handler,
    receiver: receiver::Handler,
}

impl Responder {
    pub fn new(
        connect: &RwLock<ConnectState>,
        command: &UnboundedSender<Command>,
        status: &UnboundedSender<Status>,
    ) -> Self {
        Self {
            connection: connection::Handler,
            heartbeat: heartbeat::Handler::new(command.clone()),
            media: media::Handler::new(connect.clone(), command.clone(), status.clone()),
            receiver: receiver::Handler::new(connect.clone(), command.clone(), status.clone()),
        }
    }

    pub fn handle(&self, message: &CastMessage) -> Result<(), Error> {
        // Try handlers in order of receive frequency
        if self.media.try_handle(message)?.is_none()
            && self.receiver.try_handle(message)?.is_none()
            && self.heartbeat.try_handle(message)?.is_none()
            && self.connection.try_handle(message)?.is_none()
        {
            warn!("message on unknown channel {}", message.get_namespace());
            return Err(Error::UnknownPayload);
        }
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct MessageBuilder<'a> {
    namespace: Option<&'a str>,
    source: Option<&'a str>,
    destination: Option<&'a str>,
    payload: Option<String>,
}

impl<'a> MessageBuilder<'a> {
    pub fn namespace(mut self, namespace: &'a str) -> Self {
        self.namespace = Some(namespace);
        self
    }

    pub fn source(mut self, source: &'a str) -> Self {
        self.source = Some(source);
        self
    }

    pub fn destination(mut self, destination: &'a str) -> Self {
        self.destination = Some(destination);
        self
    }

    pub fn payload<T: Serialize>(mut self, payload: &T) -> Self {
        if let Ok(payload) = serde_json::to_string(payload) {
            self.payload = Some(payload);
        }
        self
    }

    pub fn into_message(mut self) -> CastMessage {
        let mut message = CastMessage::new();
        message.set_protocol_version(CastMessage_ProtocolVersion::CASTV2_1_0);
        if let Some(source) = self.source.take() {
            message.set_source_id(source.to_owned());
        }
        if let Some(destination) = self.destination.take() {
            message.set_destination_id(destination.to_owned());
        }
        if let Some(namespace) = self.namespace.take() {
            message.set_namespace(namespace.to_owned());
        }
        if let Some(payload) = self.payload.take() {
            message.set_payload_type(CastMessage_PayloadType::STRING);
            message.set_payload_utf8(payload);
        }
        message
    }
}

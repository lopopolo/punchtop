use futures::sync::mpsc::UnboundedSender;
use serde_derive::{Deserialize, Serialize};

use crate::channel::{self, Error, MessageBuilder, DEFAULT_DESTINATION_ID, DEFAULT_SENDER_ID};
use crate::proto::CastMessage;
use crate::Command;

const CHANNEL: &str = "heartbeat";
const NAMESPACE: &str = "urn:x-cast:com.google.cast.tp.heartbeat";

#[derive(Debug)]
pub struct Handler {
    command: UnboundedSender<Command>,
}

impl Handler {
    pub fn new(command: UnboundedSender<Command>) -> Self {
        Self { command }
    }
}

impl channel::Handler for Handler {
    type Payload = Response;

    fn channel(&self) -> &str {
        CHANNEL
    }

    fn namespace(&self) -> &str {
        NAMESPACE
    }

    fn handle(&self, payload: Self::Payload) -> Result<(), Error> {
        trace!("{} got {:?}", self.channel(), payload);
        match payload {
            Response::Ping => self
                .command
                .unbounded_send(Command::Pong)
                .map_err(|_| Error::CommandSend),
            Response::Pong => Ok(()),
        }
    }
}

#[derive(Serialize, Debug)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Request {
    Ping,
    Pong,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Response {
    Ping,
    Pong,
}

pub fn ping() -> CastMessage {
    MessageBuilder::default()
        .namespace(NAMESPACE)
        .source(DEFAULT_SENDER_ID)
        .destination(DEFAULT_DESTINATION_ID)
        .payload(&Request::Ping)
        .into_message()
}

pub fn pong() -> CastMessage {
    MessageBuilder::default()
        .namespace(NAMESPACE)
        .source(DEFAULT_SENDER_ID)
        .destination(DEFAULT_DESTINATION_ID)
        .payload(&Request::Pong)
        .into_message()
}

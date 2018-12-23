use serde_derive::{Deserialize, Serialize};

use crate::channel::{self, Error, MessageBuilder, DEFAULT_SENDER_ID};
use crate::proto::CastMessage;

const CHANNEL: &str = "connection";
const NAMESPACE: &str = "urn:x-cast:com.google.cast.tp.connection";
const USER_AGENT: &str = "punchtop/cast-client";

#[derive(Debug)]
pub struct Handler;

impl channel::Handler for Handler {
    type Payload = Response;

    fn channel(&self) -> &str {
        CHANNEL
    }

    fn namespace(&self) -> &str {
        NAMESPACE
    }

    fn handle(&self, _: Self::Payload) -> Result<(), Error> {
        warn!("cast connection closed");
        Ok(())
    }
}

#[derive(Serialize, Debug)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Request {
    #[serde(rename_all = "camelCase")]
    Connect { user_agent: String },
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Response {
    Close,
}

pub fn connect(destination: &str) -> CastMessage {
    let payload = Request::Connect {
        user_agent: USER_AGENT.to_owned(),
    };
    MessageBuilder::default()
        .namespace(NAMESPACE)
        .source(DEFAULT_SENDER_ID)
        .destination(destination)
        .payload(&payload)
        .into_message()
}

use serde_json::{to_string, Error};

use super::super::payload::*;
use super::super::proto::CastMessage;

pub const NAMESPACE: &str = "urn:x-cast:com.google.cast.tp.heartbeat";

pub fn ping() -> Result<CastMessage, Error> {
    let payload = to_string(&heartbeat::Payload::Ping)?;
    Ok(super::message(NAMESPACE, payload))
}

pub fn pong() -> Result<CastMessage, Error> {
    let payload = to_string(&heartbeat::Payload::Pong)?;
    Ok(super::message(NAMESPACE, payload))
}

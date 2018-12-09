use serde_json::{to_string, Error};

use super::super::payload::*;
use super::super::proto::CastMessage;

pub const NAMESPACE: &str = "urn:x-cast:com.google.cast.tp.connection";
const USER_AGENT: &str = "Punchtop";

pub fn connect() -> Result<CastMessage, Error> {
    let payload = to_string(&connection::Payload::Connect {
        user_agent: USER_AGENT.to_owned(),
    })?;
    Ok(super::message(NAMESPACE, payload))
}

pub fn close() -> Result<CastMessage, Error> {
    let payload = to_string(&connection::Payload::Close)?;
    Ok(super::message(NAMESPACE, payload))
}

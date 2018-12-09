use serde_json::{to_string, Error};

use super::super::payload::*;
use super::super::proto::CastMessage;

pub const NAMESPACE: &str = "urn:x-cast:com.google.cast.receiver";

pub fn launch(request_id: i32, app_id: &str) -> Result<CastMessage, Error> {
    let payload = to_string(&receiver::Payload::Launch {
        request_id,
        app_id: app_id.to_owned(),
    })?;
    Ok(super::message(NAMESPACE, payload))
}

pub fn stop(
    request_id: i32,
    session_id: &str,
    ) -> Result<CastMessage, Error> {
    let payload = serde_json::to_string(&receiver::Payload::Stop {
        request_id,
        session_id: session_id.to_owned(),
    })?;
    Ok(super::message(NAMESPACE, payload))
}

pub fn status(request_id: i32) -> Result<CastMessage, Error> {
    let payload = serde_json::to_string(&receiver::Payload::GetStatus { request_id })?;
    Ok(super::message(NAMESPACE, payload))
}

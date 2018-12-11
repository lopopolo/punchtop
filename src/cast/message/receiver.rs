use serde_json::{to_string, Error};

use super::super::payload::*;
use super::super::proto::{CastMessage, CastMessage_PayloadType, CastMessage_ProtocolVersion};

pub const NAMESPACE: &str = "urn:x-cast:com.google.cast.receiver";

pub fn launch(request_id: i64, app_id: &str) -> Result<CastMessage, Error> {
    let payload = to_string(&receiver::Payload::Launch {
        request_id,
        app_id: app_id.to_owned(),
    })?;
    Ok(message(super::DEFAULT_DESTINATION_ID, payload))
}

pub fn status(request_id: i64) -> Result<CastMessage, Error> {
    let payload = serde_json::to_string(&receiver::Payload::GetStatus { request_id })?;
    Ok(message(super::DEFAULT_DESTINATION_ID, payload))
}

fn message(dest: &str, payload: String) -> CastMessage {
    let mut msg = CastMessage::new();
    msg.set_payload_type(CastMessage_PayloadType::STRING);
    msg.set_protocol_version(CastMessage_ProtocolVersion::CASTV2_1_0);
    msg.set_namespace(NAMESPACE.to_owned());
    msg.set_source_id(super::DEFAULT_SENDER_ID.to_owned());
    msg.set_destination_id(dest.to_owned());
    msg.set_payload_utf8(payload);
    msg
}

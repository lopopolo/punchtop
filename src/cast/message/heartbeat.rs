use serde_json::{to_string, Error};

use crate::cast::payload::heartbeat::*;
use crate::cast::proto::{CastMessage, CastMessage_PayloadType, CastMessage_ProtocolVersion};

pub const NAMESPACE: &str = "urn:x-cast:com.google.cast.tp.heartbeat";

pub fn ping() -> Result<CastMessage, Error> {
    let payload = to_string(&Request::Ping)?;
    Ok(message(payload))
}

pub fn pong() -> Result<CastMessage, Error> {
    let payload = to_string(&Request::Pong)?;
    Ok(message(payload))
}

fn message(payload: String) -> CastMessage {
    let mut msg = CastMessage::new();
    msg.set_payload_type(CastMessage_PayloadType::STRING);
    msg.set_protocol_version(CastMessage_ProtocolVersion::CASTV2_1_0);
    msg.set_namespace(NAMESPACE.to_owned());
    msg.set_source_id(super::DEFAULT_SENDER_ID.to_owned());
    msg.set_destination_id(super::DEFAULT_DESTINATION_ID.to_owned());
    msg.set_payload_utf8(payload);
    msg
}

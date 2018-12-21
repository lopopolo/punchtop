use serde_json::{to_string, Error};

use crate::payload::connection::*;
use crate::proto::{CastMessage, CastMessage_PayloadType, CastMessage_ProtocolVersion};

pub const NAMESPACE: &str = "urn:x-cast:com.google.cast.tp.connection";
const USER_AGENT: &str = "Punchtop";

pub fn connect(dest: &str) -> Result<CastMessage, Error> {
    let payload = to_string(&Request::Connect {
        user_agent: USER_AGENT.to_owned(),
    })?;
    Ok(message(dest, payload))
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

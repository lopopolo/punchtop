use protobuf::{CodedOutputStream, ProtobufResult};

use super::proto::{CastMessage, CastMessage_PayloadType, CastMessage_ProtocolVersion};

pub mod connection;
pub mod heartbeat;
pub mod media;
pub mod receiver;

pub mod namespace {
    pub const CONNECTION: &str = super::connection::NAMESPACE;
    pub const HEARTBEAT: &str = super::heartbeat::NAMESPACE;
    pub const MEDIA: &str = super::media::NAMESPACE;
    pub const RECEIVER: &str = super::receiver::NAMESPACE;
}

const DEFAULT_SENDER_ID: &str = "sender-0";
const DEFAULT_DESTINATION_ID: &str = "receiver-0";

pub fn encode(msg: impl protobuf::Message, buf: &mut Vec<u8>) -> ProtobufResult<()> {
    let mut output = CodedOutputStream::new(buf);
    msg.write_to(&mut output)?;
    output.flush()
}

fn message(namespace: &str, payload: String) -> CastMessage {
    let mut msg = CastMessage::new();
    msg.set_payload_type(CastMessage_PayloadType::STRING);
    msg.set_protocol_version(CastMessage_ProtocolVersion::CASTV2_1_0);
    msg.set_namespace(namespace.to_owned());
    msg.set_source_id(DEFAULT_SENDER_ID.to_owned());
    msg.set_destination_id(DEFAULT_DESTINATION_ID.to_owned());
    msg.set_payload_utf8(payload);
    msg
}

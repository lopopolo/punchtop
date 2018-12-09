use protobuf::{CodedOutputStream, ProtobufResult};
use serde_json::Error;

use super::payload::*;
use super::proto::{CastMessage, CastMessage_PayloadType, CastMessage_ProtocolVersion};
use super::provider::*;

const DEFAULT_SENDER_ID: &str = "sender-0";
const DEFAULT_DESTINATION_ID: &str = "receiver-0";

pub fn encode(msg: impl protobuf::Message, buf: &mut Vec<u8>) -> ProtobufResult<()> {
    let mut output = CodedOutputStream::new(buf);
    msg.write_to(&mut output)?;
    output.flush()
}

pub fn connect() -> Result<CastMessage, Error> {
    let payload = serde_json::to_string(&connection::Payload::Connect)?;
    Ok(message(connection::NAMESPACE, payload))
}

pub fn close() -> Result<CastMessage, Error> {
    let payload = serde_json::to_string(&connection::Payload::Close)?;
    Ok(message(connection::NAMESPACE, payload))
}

pub fn load(request_id: i32, session_id: &str, media: Media) -> Result<CastMessage, Error> {
    let media = {
        let config = media::Media {
            content_id: media.url.to_string(),
            stream_type: "NONE".to_string(),
            content_type: media.content_type,
            metadata: None,
            duration: None,
        };
        config
    };
    let payload = serde_json::to_string(&media::Payload::Load {
        request_id,
        session_id: session_id.to_owned(),
        media,
        current_time: 0f32,
        custom_data: media::CustomData::new(),
        autoplay: true,
    })?;
    Ok(message(media::NAMESPACE, payload))
}

pub fn play(request_id: i32, media_session_id: i32) -> Result<CastMessage, Error> {
    let payload = serde_json::to_string(&media::Payload::Play {
        request_id,
        media_session_id: media_session_id,
        custom_data: media::CustomData::new(),
    })?;
    Ok(message(media::NAMESPACE, payload))
}

pub fn ping() -> Result<CastMessage, Error> {
    let namespace = "urn:x-cast:com.google.cast.tp.heartbeat";
    let payload = serde_json::to_string(&heartbeat::Payload::Ping)?;
    Ok(message(heartbeat::NAMESPACE, payload))
}

pub fn pong() -> Result<CastMessage, Error> {
    let payload = serde_json::to_string(&heartbeat::Payload::Pong)?;
    Ok(message(heartbeat::NAMESPACE, payload))
}

pub fn launch(request_id: i32, app_id: &str) -> Result<CastMessage, Error> {
    let payload = serde_json::to_string(&receiver::Payload::Launch {
        request_id,
        app_id: app_id.to_owned(),
    })?;
    Ok(message(receiver::NAMESPACE, payload))
}

pub fn stop(
    request_id: i32,
    session_id: &str,
    ) -> Result<CastMessage, Error> {
    let payload = serde_json::to_string(&receiver::Payload::Stop {
        request_id,
        session_id: session_id.to_owned(),
    })?;
    Ok(message(receiver::NAMESPACE, payload))
}

pub fn receiver_status(request_id: i32) -> Result<CastMessage, Error> {
    let payload = serde_json::to_string(&receiver::Payload::GetStatus { request_id })?;
    Ok(message(receiver::NAMESPACE, payload))
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

use serde_json::{to_string, Error};

use super::super::payload::*;
use super::super::proto::{CastMessage, CastMessage_PayloadType, CastMessage_ProtocolVersion};
use super::super::provider::Media;

pub const NAMESPACE: &str = "urn:x-cast:com.google.cast.media";

pub fn load(request_id: i32, session_id: &str, transport_id: &str, media: Media) -> Result<CastMessage, Error> {
    let media = {
        let config = media::MediaInformation {
            content_id: media.url.to_string(),
            stream_type: media::StreamType::None,
            content_type: media.content_type,
            metadata: None,
            duration: None,
        };
        config
    };
    let payload = to_string(&media::Payload::Load {
        request_id,
        session_id: session_id.to_owned(),
        media,
        current_time: 0f32,
        custom_data: media::CustomData::new(),
        autoplay: true,
    })?;
    Ok(message(transport_id, payload))
}

pub fn play(request_id: i32, transport_id: &str, media_session_id: i32) -> Result<CastMessage, Error> {
    let payload = to_string(&media::Payload::Play {
        request_id,
        media_session_id: media_session_id,
        custom_data: media::CustomData::new(),
    })?;
    Ok(message(transport_id, payload))
}

pub fn status(request_id: i32, transport_id: &str) -> Result<CastMessage, Error> {
    let payload = to_string(&media::Payload::GetStatus {
        request_id,
        media_session_id: None,
    })?;
    Ok(message(transport_id, payload))
}

fn message(transport_id: &str, payload: String) -> CastMessage {
    let mut msg = CastMessage::new();
    msg.set_payload_type(CastMessage_PayloadType::STRING);
    msg.set_protocol_version(CastMessage_ProtocolVersion::CASTV2_1_0);
    msg.set_namespace(NAMESPACE.to_owned());
    msg.set_source_id(super::DEFAULT_SENDER_ID.to_owned());
    msg.set_destination_id(transport_id.to_owned());
    msg.set_payload_utf8(payload);
    msg
}

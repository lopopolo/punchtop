use serde_json::{to_string, Error};

use super::super::payload::*;
use super::super::proto::CastMessage;
use super::super::provider::Media;

pub const NAMESPACE: &str = "urn:x-cast:com.google.cast.media";

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
    let payload = to_string(&media::Payload::Load {
        request_id,
        session_id: session_id.to_owned(),
        media,
        current_time: 0f32,
        custom_data: media::CustomData::new(),
        autoplay: true,
    })?;
    Ok(super::message(NAMESPACE, payload))
}

pub fn play(request_id: i32, media_session_id: i32) -> Result<CastMessage, Error> {
    let payload = serde_json::to_string(&media::Payload::Play {
        request_id,
        media_session_id: media_session_id,
        custom_data: media::CustomData::new(),
    })?;
    Ok(super::message(NAMESPACE, payload))
}

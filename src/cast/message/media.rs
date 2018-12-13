use serde_json::{to_string, Error};

use cast::payload::*;
use cast::proto::{CastMessage, CastMessage_PayloadType, CastMessage_ProtocolVersion};
use cast::provider::{Media, MediaConnection, ReceiverConnection};

pub const NAMESPACE: &str = "urn:x-cast:com.google.cast.media";

pub fn load(
    request_id: i64,
    connect: &ReceiverConnection,
    media: Media,
) -> Result<CastMessage, Error> {
    let mut metadata = media::Metadata::music_default();
    metadata.title = media.title;
    metadata.artist = media.artist;
    metadata.album_name = media.album;
    if let Some(image) = media.cover {
        metadata.images.push(media::Image {
            url: image.url.to_string(),
            width: Some(image.dimensions.0),
            height: Some(image.dimensions.1),
        });
    }
    let media = media::MediaInformation {
        content_id: media.url.to_string(),
        stream_type: media::StreamType::None,
        content_type: media.content_type,
        metadata: Some(metadata),
        duration: None,
    };
    let payload = to_string(&media::Payload::Load {
        request_id,
        session_id: connect.session.to_owned(),
        media,
        current_time: 0f64,
        custom_data: media::CustomData::default(),
        autoplay: true,
    })?;
    Ok(message(&connect.transport, payload))
}

pub fn play(request_id: i64, connect: &MediaConnection) -> Result<CastMessage, Error> {
    let payload = to_string(&media::Payload::Play {
        request_id,
        media_session_id: connect.session,
        custom_data: media::CustomData::default(),
    })?;
    Ok(message(&connect.receiver.transport, payload))
}

pub fn status(request_id: i64, connect: &MediaConnection) -> Result<CastMessage, Error> {
    let payload = to_string(&media::Payload::GetStatus {
        request_id,
        media_session_id: Some(connect.session),
    })?;
    Ok(message(&connect.receiver.transport, payload))
}

pub fn stop(request_id: i64, connect: &MediaConnection) -> Result<CastMessage, Error> {
    let payload = serde_json::to_string(&media::Payload::Stop {
        request_id,
        media_session_id: connect.session,
        custom_data: media::CustomData::default(),
    })?;
    Ok(message(&connect.receiver.transport, payload))
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

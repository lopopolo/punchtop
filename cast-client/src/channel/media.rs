use futures::sync::mpsc::UnboundedSender;
use futures::Future;
use futures_locks::RwLock;
use serde;
use serde_derive::{Deserialize, Serialize};

use crate::channel::{self, Error, MessageBuilder, DEFAULT_SENDER_ID};
use crate::proto::CastMessage;
use crate::provider::{Media, MediaConnection, ReceiverConnection};
use crate::session;
use crate::{Command, ConnectState, Status};

const CHANNEL: &str = "media";
const METADATA_TYPE_MUSIC_TRACK: u32 = 3;
const NAMESPACE: &str = "urn:x-cast:com.google.cast.media";

#[derive(Debug)]
pub struct Handler {
    connect: RwLock<ConnectState>,
    command: UnboundedSender<Command>,
    status: UnboundedSender<Status>,
}

impl Handler {
    pub fn new(
        connect: RwLock<ConnectState>,
        command: UnboundedSender<Command>,
        status: UnboundedSender<Status>,
    ) -> Self {
        Self {
            connect,
            command,
            status,
        }
    }
}

impl channel::Handler for Handler {
    type Payload = Response;

    fn channel(&self) -> &str {
        CHANNEL
    }

    fn namespace(&self) -> &str {
        NAMESPACE
    }

    fn handle(&self, payload: Self::Payload) -> Result<(), Error> {
        match payload {
            Response::MediaStatus { status, .. } => {
                let status = status.into_iter().next();
                let session = status.as_ref().map(|status| status.media_session_id);
                if let Some(session) = session {
                    let tx = self.status.clone();
                    let task = session::register(&self.connect, session);
                    let task = task.and_then(move |connect| {
                        if let Some(connect) = connect {
                            tx.unbounded_send(Status::MediaConnected(Box::new(connect)))
                                .map_err(|_| ())?;
                        }
                        Ok(())
                    });
                    tokio_executor::spawn(task);
                } else {
                    tokio_executor::spawn(session::invalidate(&self.connect));
                }
                if let (Some(state), false) = (status, self.status.is_closed()) {
                    self.status
                        .unbounded_send(Status::MediaState(Box::new(state)))
                        .map_err(|_| Error::StatusSend)?;
                }
                Ok(())
            }
            _ => Err(Error::UnknownPayload),
        }
    }
}

#[derive(Serialize, Debug)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
#[allow(clippy::large_enum_variant)]
pub enum Request<CustomData: serde::Serialize> {
    #[serde(rename_all = "camelCase")]
    Load {
        request_id: i64,
        session_id: String,
        media: MediaInformation,
        #[serde(skip_serializing_if = "Option::is_none")]
        autoplay: Option<bool>, // defaults to true
        #[serde(skip_serializing_if = "Option::is_none")]
        current_time: Option<f64>, // only optional if content is live content
        #[serde(skip_serializing_if = "Option::is_none")]
        custom_data: Option<CustomData>,
    },
    #[serde(rename_all = "camelCase")]
    Pause {
        media_session_id: i64,
        request_id: i64,
        #[serde(skip_serializing_if = "Option::is_none")]
        custom_data: Option<CustomData>,
    },
    #[serde(rename_all = "camelCase")]
    #[allow(dead_code)]
    Seek {
        media_session_id: i64,
        request_id: i64,
        #[serde(skip_serializing_if = "Option::is_none")]
        resume_state: Option<ResumeState>, // playback status will not change if not set
        #[serde(skip_serializing_if = "Option::is_none")]
        current_time: Option<f64>, // optional if the content is live content
        #[serde(skip_serializing_if = "Option::is_none")]
        custom_data: Option<CustomData>,
    },
    #[serde(rename_all = "camelCase")]
    Stop {
        media_session_id: i64,
        request_id: i64,
        #[serde(skip_serializing_if = "Option::is_none")]
        custom_data: Option<CustomData>,
    },
    #[serde(rename_all = "camelCase")]
    Play {
        media_session_id: i64,
        request_id: i64,
        #[serde(skip_serializing_if = "Option::is_none")]
        custom_data: Option<CustomData>,
    },
    #[serde(rename_all = "camelCase")]
    GetStatus {
        #[serde(skip_serializing_if = "Option::is_none")]
        media_session_id: Option<i64>,
        request_id: i64,
        #[serde(skip_serializing_if = "Option::is_none")]
        custom_data: Option<CustomData>,
    },
    #[serde(rename_all = "camelCase")]
    #[allow(dead_code)]
    // Media stream volume (distinct from device volume)
    Volume {
        media_session_id: Option<i64>,
        request_id: i64,
        volume: Volume,
        #[serde(skip_serializing_if = "Option::is_none")]
        custom_data: Option<CustomData>,
    },
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Response {
    #[serde(rename_all = "camelCase")]
    MediaStatus {
        #[serde(default)]
        request_id: i64,
        status: Vec<MediaStatus>,
    },
    #[serde(rename_all = "camelCase")]
    LoadCancelled { request_id: i64 },
    #[serde(rename_all = "camelCase")]
    LoadFailed { request_id: i64 },
    #[serde(rename_all = "camelCase")]
    InvalidPlayerState { request_id: i64 },
    #[serde(rename_all = "camelCase")]
    InvalidRequest {
        request_id: i64,
        reason: Option<String>,
    },
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ResumeState {
    PlaybackStart,
    PlaybackPause,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::module_name_repetitions)]
pub struct MediaInformation {
    pub content_id: String,
    pub stream_type: StreamType,
    pub content_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Metadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<f64>,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StreamType {
    None,
    Buffered,
    Live,
}

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
    pub metadata_type: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub series_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub album_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub album_artist: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub composer: Option<String>,
    pub images: Vec<Image>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_air_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_date_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub studio: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latitude: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub longitude: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub season: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub episode: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub track_number: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disc_number: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
}

impl Metadata {
    pub fn music_default() -> Self {
        Self {
            metadata_type: METADATA_TYPE_MUSIC_TRACK,
            ..Self::default()
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Image {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Volume {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub muted: Option<bool>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::module_name_repetitions)]
pub struct MediaStatus {
    pub media_session_id: i64,
    #[serde(default)]
    pub media: Option<MediaInformation>,
    pub playback_rate: f64,
    pub player_state: PlayerState,
    pub idle_reason: Option<IdleReason>,
    pub current_time: f64,
    pub supported_media_commands: u32,
}

#[derive(Deserialize, Debug, Eq, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PlayerState {
    Idle,
    Playing,
    Buffering,
    Paused,
}

#[derive(Deserialize, Debug, Eq, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IdleReason {
    Cancelled,
    Interrupted,
    Finished,
    Error,
}

pub fn load(request_id: i64, connect: &ReceiverConnection, media: Media) -> CastMessage {
    let mut images = Vec::with_capacity(1);
    if let Some(image) = media.cover {
        images.push(Image {
            url: image.url.to_string(),
            width: Some(image.dimensions.0),
            height: Some(image.dimensions.1),
        });
    }
    let metadata = Metadata {
        title: media.title,
        artist: media.artist,
        album_name: media.album,
        images,
        ..Metadata::music_default()
    };
    let media = MediaInformation {
        content_id: media.url.to_string(),
        stream_type: StreamType::None, // let the device decide whether to buffer
        content_type: media.content_type,
        metadata: Some(metadata),
        duration: media.duration,
    };
    let payload: Request<()> = Request::Load {
        request_id,
        session_id: connect.session.to_owned(),
        media,
        current_time: None,
        custom_data: None,
        autoplay: None,
    };
    MessageBuilder::default()
        .namespace(NAMESPACE)
        .source(DEFAULT_SENDER_ID)
        .destination(&connect.transport)
        .payload(&payload)
        .into_message()
}

pub fn pause(request_id: i64, connect: &MediaConnection) -> CastMessage {
    let payload: Request<()> = Request::Pause {
        media_session_id: connect.session,
        request_id,
        custom_data: None,
    };
    MessageBuilder::default()
        .namespace(NAMESPACE)
        .source(DEFAULT_SENDER_ID)
        .destination(&connect.receiver.transport)
        .payload(&payload)
        .into_message()
}

pub fn play(request_id: i64, connect: &MediaConnection) -> CastMessage {
    let payload: Request<()> = Request::Play {
        media_session_id: connect.session,
        request_id,
        custom_data: None,
    };
    MessageBuilder::default()
        .namespace(NAMESPACE)
        .source(DEFAULT_SENDER_ID)
        .destination(&connect.receiver.transport)
        .payload(&payload)
        .into_message()
}

pub fn status(request_id: i64, connect: &MediaConnection) -> CastMessage {
    let payload: Request<()> = Request::GetStatus {
        media_session_id: Some(connect.session),
        request_id,
        custom_data: None,
    };
    MessageBuilder::default()
        .namespace(NAMESPACE)
        .source(DEFAULT_SENDER_ID)
        .destination(&connect.receiver.transport)
        .payload(&payload)
        .into_message()
}

pub fn stop(request_id: i64, connect: &MediaConnection) -> CastMessage {
    let payload: Request<()> = Request::Stop {
        media_session_id: connect.session,
        request_id,
        custom_data: None,
    };
    MessageBuilder::default()
        .namespace(NAMESPACE)
        .source(DEFAULT_SENDER_ID)
        .destination(&connect.receiver.transport)
        .payload(&payload)
        .into_message()
}

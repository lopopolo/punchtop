//! The media channel manages media sessions and playback. See the
//! [cast reference docs](https://developers.google.com/cast/docs/reference/messages).

use serde_derive::{Deserialize, Serialize};

const METADATA_TYPE_MUSIC_TRACK: u32 = 3;

#[derive(Serialize, Debug)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Request {
    #[serde(rename_all = "camelCase")]
    GetStatus {
        request_id: i64,
        #[serde(skip_serializing_if = "Option::is_none")]
        media_session_id: Option<i64>,
    },
    #[serde(rename_all = "camelCase")]
    Load {
        request_id: i64,
        session_id: String,
        media: MediaInformation,
        current_time: f64,
        custom_data: CustomData,
        autoplay: bool,
    },
    #[serde(rename_all = "camelCase")]
    Play {
        request_id: i64,
        media_session_id: i64,
        custom_data: CustomData,
    },
    #[serde(rename_all = "camelCase")]
    Pause {
        request_id: i64,
        media_session_id: i64,
        custom_data: CustomData,
    },
    #[serde(rename_all = "camelCase")]
    Stop {
        request_id: i64,
        media_session_id: i64,
        custom_data: CustomData,
    },
    #[serde(rename_all = "camelCase")]
    Seek {
        request_id: i64,
        media_session_id: i64,
        resume_state: Option<ResumeState>,
        current_time: Option<f64>,
        custom_data: CustomData,
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

#[derive(Serialize, Debug, Default)]
pub struct CustomData {
    #[serde(skip_serializing)]
    private: (),
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
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

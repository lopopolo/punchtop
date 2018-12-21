//! The receiver channel manages global receiver state like the active cast app
//! and device volume.

use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Debug)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Request {
    #[serde(rename_all = "camelCase")]
    Launch {
        request_id: i64,
        app_id: String,
    },
    #[serde(rename_all = "camelCase")]
    GetStatus {
        request_id: i64,
    },
    #[serde(rename_all = "camelCase")]
    GetAppAvailability {
        request_id: i64,
        app_id: Vec<String>,
    },
    SetVolume {
        volume: Volume,
    },
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Response {
    #[serde(rename_all = "camelCase")]
    ReceiverStatus { request_id: i64, status: Status },
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    #[serde(default)]
    pub applications: Vec<Applications>,
    #[serde(default)]
    pub is_active_input: bool,
    pub volume: Volume,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Applications {
    pub app_id: String,
    pub display_name: String,
    pub namespaces: Vec<Namespace>,
    pub session_id: String,
    pub status_text: String,
    pub transport_id: String,
}

#[derive(Deserialize, Debug)]
pub struct Namespace {
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Volume {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub muted: Option<bool>,
}

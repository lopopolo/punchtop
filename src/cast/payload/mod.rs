use std::vec::Vec;

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Payload {
    Connect {},
    Close {},
    Ping {},
    Pong {},
    #[serde(rename_all = "camelCase")]
    Launch { request_id: i64, app_id: String },
    #[serde(rename_all = "camelCase")]
    Stop { request_id: i64, session_id: String },
    GetStatus { request_id: i64 },
    #[serde(rename_all = "camelCase")]
    GetAppAvailability { request_id: i64, app_id: Vec<String> },
    #[serde(rename_all = "camelCase")]
    ReceiverStatus { request_id: i64, status: Status },
    SetVolume { volume: Volume },
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    #[serde(default)]
    pub applications: Option<Vec<Applications>>,
    #[serde(default)]
    pub is_active_input: Option<bool>,
    pub volume: Volume,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Applications {
    pub app_id: String,
    pub display_name: String,
    pub namespaces: Vec<Namespace>,
    pub session_id: String,
    pub status_text: String,
    pub transport_id: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Namespace {
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Volume {
    pub level: f32,
    pub muted: bool,
}

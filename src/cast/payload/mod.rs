use std::vec::Vec;

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Payload {
    Connect,
    Close,
    Ping,
    Pong,
    #[serde(rename_all = "camelCase")]
    Launch { request_id: i32, app_id: String },
    #[serde(rename_all = "camelCase")]
    Stop { request_id: i32, session_id: String },
    #[serde(rename_all = "camelCase")]
    GetStatus { request_id: i32 },
    #[serde(rename_all = "camelCase")]
    GetAppAvailability { request_id: i32, app_id: Vec<String> },
    #[serde(rename_all = "camelCase")]
    ReceiverStatus { request_id: i32, status: Status },
    SetVolume { volume: Volume },
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    #[serde(default)]
    pub applications: Vec<Applications>,
    #[serde(default)]
    pub is_active_input: bool,
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

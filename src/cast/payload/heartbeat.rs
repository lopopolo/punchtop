//! The heartbeat channel defines one request, `Ping`, and response, `Pong`,
//! for connection keepalive.

#[derive(Serialize, Debug)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Request {
    Ping,
    Pong,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Response {
    Ping,
    Pong,
}

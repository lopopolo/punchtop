//! The connection channel manages connection state to cast transports.

use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Debug)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Request {
    #[serde(rename_all = "camelCase")]
    Connect { user_agent: String },
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Response {
    Close,
}

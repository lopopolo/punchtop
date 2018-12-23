use futures::sync::mpsc::UnboundedSender;
use futures::Future;
use futures_locks::RwLock;
use serde_derive::{Deserialize, Serialize};

use crate::channel::{
    self, Error, MessageBuilder, DEFAULT_DESTINATION_ID, DEFAULT_MEDIA_RECEIVER_APP_ID,
    DEFAULT_SENDER_ID,
};
use crate::proto::CastMessage;
use crate::{Command, ConnectState};

const CHANNEL: &str = "receiver";
const NAMESPACE: &str = "urn:x-cast:com.google.cast.receiver";

#[derive(Debug)]
pub struct Handler {
    connect: RwLock<ConnectState>,
    command: UnboundedSender<Command>,
    status: UnboundedSender<crate::Status>,
}

impl Handler {
    pub fn new(
        connect: RwLock<ConnectState>,
        command: UnboundedSender<Command>,
        status: UnboundedSender<crate::Status>,
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
        let Response::ReceiverStatus { status, .. } = payload;
        let app = status
            .applications
            .iter()
            .find(|app| app.app_id == DEFAULT_MEDIA_RECEIVER_APP_ID);
        let session = app.map(|app| app.session_id.to_owned());
        let transport = app.map(|app| app.transport_id.to_owned());
        let status = self.status.clone();
        let command = self.command.clone();
        let connect = self.connect.write().and_then(move |mut state| {
            trace!("acquired connect state lock in receiver channel");
            if !state.set_session(session.deref()) || !state.set_transport(transport.deref()) {
                // Connection did not change
                return Ok(());
            }
            if let Some(ref connect) = state.receiver_connection() {
                debug!("connecting to transport {}", connect.transport);
                status
                    .unbounded_send(crate::Status::Connected(Box::new(connect.clone())))
                    .map_err(|_| ())?;
                // we've connected to the default receiver. Now connect to the
                // transport backing the launched app session.
                command
                    .unbounded_send(Command::Connect(connect.clone()))
                    .map_err(|_| ())?;
            }
            Ok(())
        });
        tokio_executor::spawn(connect);
        Ok(())
    }
}

#[derive(Serialize, Debug)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Request {
    #[serde(rename_all = "camelCase")]
    Launch { request_id: i64, app_id: String },
    #[serde(rename_all = "camelCase")]
    GetStatus { request_id: i64 },
    #[serde(rename_all = "camelCase")]
    #[allow(dead_code)]
    GetAppAvailability {
        request_id: i64,
        app_id: Vec<String>,
    },
    #[allow(dead_code)]
    SetVolume { volume: Volume },
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

pub fn launch(request_id: i64, app_id: &str) -> CastMessage {
    let payload = Request::Launch {
        request_id,
        app_id: app_id.to_owned(),
    };
    MessageBuilder::default()
        .namespace(NAMESPACE)
        .source(DEFAULT_SENDER_ID)
        .destination(DEFAULT_DESTINATION_ID)
        .payload(&payload)
        .into_message()
}

pub fn status(request_id: i64) -> CastMessage {
    let payload = Request::GetStatus { request_id };
    MessageBuilder::default()
        .namespace(NAMESPACE)
        .source(DEFAULT_SENDER_ID)
        .destination(DEFAULT_DESTINATION_ID)
        .payload(&payload)
        .into_message()
}

use std::error;
use std::fmt;

use url::Url;

use super::payload::media::MediaStatus;

#[derive(Clone, Debug)]
pub struct Media {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub url: Url,
    pub cover: Option<Image>,
    pub content_type: String,
}

impl fmt::Display for Media {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut parts = Vec::new();
        if let Some(ref artist) = self.artist {
            parts.push(artist.clone());
        }
        if let Some(ref title) = self.title {
            parts.push(title.clone());
        }
        if let Some(ref album) = self.album {
            parts.push(album.clone());
        }
        write!(f, "{}", parts.join(" -- "))
    }
}

#[derive(Clone, Debug)]
pub struct Image {
    pub url: Url,
    pub dimensions: (u32, u32),
}

#[derive(Debug)]
pub enum Error {
    UnknownChannel(String),
}

impl error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::UnknownChannel(ref channel) => {
                write!(f, "Message received on unknown channel {:?}", channel)
            }
        }
    }
}

#[derive(Debug)]
pub enum Command {
    Close(ReceiverConnection),
    Connect(ReceiverConnection),
    Heartbeat,
    Launch {
        app_id: String,
    },
    Load {
        connect: ReceiverConnection,
        media: Box<Media>,
    },
    MediaStatus(MediaConnection),
    Pause(MediaConnection),
    Play(MediaConnection),
    ReceiverStatus,
    Seek(MediaConnection, f32),
    Shutdown,
    Stop(MediaConnection),
    VolumeLevel(MediaConnection, f32),
    VolumeMute(MediaConnection, bool),
}

#[derive(Debug)]
pub enum Status {
    Connected(Box<ReceiverConnection>),
    MediaConnected(Box<MediaConnection>),
    MediaStatus(Box<MediaStatus>),
    LoadCancelled,
    LoadFailed,
    InvalidPlayerState,
    InvalidRequest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SessionLifecycle {
    Init,
    Established,
    NoMediaSession,
}

impl Default for SessionLifecycle {
    fn default() -> SessionLifecycle {
        SessionLifecycle::Init
    }
}

#[derive(Debug, Default)]
pub struct ConnectState {
    session: Option<String>,
    transport: Option<String>,
    media_session: Option<i64>,
    pub lifecycle: SessionLifecycle,
}

impl ConnectState {
    pub fn receiver_connection(&self) -> Option<ReceiverConnection> {
        match (self.session.as_ref(), self.transport.as_ref()) {
            (Some(session), Some(transport)) => Some(ReceiverConnection {
                session: session.to_owned(),
                transport: transport.to_owned(),
            }),
            _ => None,
        }
    }

    pub fn media_connection(&self) -> Option<MediaConnection> {
        match self.lifecycle {
            SessionLifecycle::Init => None,
            SessionLifecycle::Established => {
                match (self.receiver_connection(), self.media_session) {
                    (Some(receiver), Some(session)) => Some(MediaConnection { receiver, session }),
                    _ => None,
                }
            }
            SessionLifecycle::NoMediaSession => None,
        }
    }

    pub fn set_session(&mut self, session: Option<&str>) -> bool {
        let mut changed = false;
        if self.session.deref() != session {
            changed = true;
            self.session = session.map(String::from);
        }
        changed
    }

    pub fn set_transport(&mut self, transport: Option<&str>) -> bool {
        let mut changed = false;
        if self.transport.deref() != transport {
            changed = true;
            self.transport = transport.map(String::from);
        }
        changed
    }

    pub fn set_media_session(&mut self, media_session: Option<i64>) -> bool {
        let mut changed = false;
        if self.media_session != media_session {
            changed = true;
            self.media_session = media_session;
        }
        changed
    }
}

#[derive(Clone, Debug)]
pub struct ReceiverConnection {
    pub session: String,
    pub transport: String,
}

#[derive(Clone, Debug)]
pub struct MediaConnection {
    pub receiver: ReceiverConnection,
    pub session: i64,
}

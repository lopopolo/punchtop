use std::error;
use std::fmt;

use url::Url;

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
    Close { destination: String },
    Connect { destination: String },
    Heartbeat,
    Launch { app_id: String },
    Load { session: String, transport: String, media: Media },
    MediaStatus { transport: String },
    Pause,
    Play { media_session: i32, transport: String },
    ReceiverStatus,
    Seek(f32),
    Stop { media_session: String, transport: String },
    VolumeLevel(f32),
    VolumeMute(bool),
}

#[derive(Debug)]
pub enum Status {
    Connected {
        transport: String,
        session: String,
        media_session: i32,
    },
    Media,
    MediaConnected(i32),
    LoadCancelled,
    LoadFailed,
    InvalidPlayerState,
    InvalidRequest,
}

#[derive(Debug, Default)]
pub struct ConnectState {
    pub session: Option<String>,
    pub transport: Option<String>,
    pub media_session: Option<i32>,
}

impl ConnectState {
    pub fn is_connected(&self) -> bool {
        self.session.is_some() && self.transport.is_some()
    }

    pub fn media_connection(&self) -> Option<MediaConnection> {
        match (self.transport.as_ref(), self.session.as_ref(), self.media_session) {
            (Some(transport), Some(session), Some(media_session)) => Some(MediaConnection {
                transport: transport.to_owned(),
                session: session.to_owned(),
                media_session: media_session,
            }),
            _ => None,
        }
    }
}

pub struct MediaConnection {
    pub transport: String,
    pub session: String,
    pub media_session: i32,
}

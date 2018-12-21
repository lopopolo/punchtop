use std::time::Duration;

use base64;
use cast_client::{MediaConnection, ReceiverConnection, Status};
use floating_duration::TimeAsFloat;
use futures::sync::oneshot;
use punchtop_audio::chromecast::{CastAddr, Device as CastDevice};
use punchtop_audio::Track;
use punchtop_playlist::fs::{FsTrack, Playlist};
use serde_derive::Serialize;
use stream_util::{DrainListener, DrainTrigger};

pub struct State {
    playlist: Playlist,
    client: Option<CastDevice>,
    connect: Option<ReceiverConnection>,
    session: Option<MediaConnection>,
    shutdown: Option<DrainTrigger>,
    devices: Vec<Device>,
}

#[derive(Debug, Eq, PartialEq)]
pub enum Lifecycle {
    Uninitialized,
    Loaded,
    Terminating,
}

pub struct Config {
    pub duration: Duration,
    pub iterations: u64,
}

pub struct Controller {
    pub config: Config,
    lifecycle: Lifecycle,
    state: State,
    events: Vec<Event>,
}

impl Controller {
    pub fn new(config: Config, playlist: Playlist) -> (Self, DrainListener) {
        let (trigger, listener) = oneshot::channel();
        let state = State {
            playlist,
            client: None,
            connect: None,
            session: None,
            shutdown: Some(trigger),
            devices: vec![],
        };
        let events = vec![];
        (
            Self {
                config,
                lifecycle: Lifecycle::Uninitialized,
                state,
                events,
            },
            listener,
        )
    }
}

impl Controller {
    pub fn set_devices(&mut self, devices: Vec<Device>) {
        std::mem::replace(&mut self.state.devices, devices);
    }

    pub fn devices(&self) -> &[Device] {
        &self.state.devices
    }

    pub fn set_client(&mut self, client: CastDevice) {
        if let Some(mut old) = std::mem::replace(&mut self.state.client, Some(client)) {
            let _ = old.shutdown();
        }
    }

    pub fn playlist_name(&self) -> &str {
        self.state.playlist.name()
    }
}

// View lifecyle
impl Controller {
    pub fn view_did_load(&mut self) {
        self.lifecycle = Lifecycle::Loaded;
    }

    pub fn view_lifecycle(&self) -> &Lifecycle {
        &self.lifecycle
    }
}

// Playback controls
impl Controller {
    fn load_next(&mut self) -> Option<(u64, FsTrack)> {
        let client = self.state.client.as_ref()?;
        let connect = self.state.connect.as_ref()?;
        self.state.playlist.next().map(|(cursor, track)| {
            let _ = client.load(&connect, &track);
            (cursor, track)
        })
    }

    pub fn pause(&self) {
        if let Some(ref client) = self.state.client {
            if let Some(ref session) = self.state.session {
                let _ = client.pause(session);
            }
        }
    }

    pub fn play(&self) {
        if let Some(ref client) = self.state.client {
            if let Some(ref session) = self.state.session {
                let _ = client.play(session);
            }
        }
    }

    fn shutdown(&mut self) {
        if let Some(ref mut client) = self.state.client {
            if let Some(ref session) = self.state.session {
                let _ = client.stop(session);
            }
            let _ = client.shutdown();
        }
        if let Some(shutdown) = self.state.shutdown.take() {
            let _ = shutdown.send(());
        }
        self.lifecycle = Lifecycle::Terminating;
    }
}

impl Controller {
    pub fn handle(&mut self, event: Status) -> Vec<Event> {
        use cast_client::Status::*;
        if !self.events.is_empty() {
            debug!("app backlog of {} events", self.events.len());
        }
        match event {
            Connected(connect) => {
                self.state.connect = Some(*connect);
                if let Some((cursor, track)) = self.load_next() {
                    self.events.push(Event::SetMedia {
                        media: media(&track, cursor),
                    });
                    self.events.push(Event::SetPlayback { is_playing: true });
                }
            }
            MediaConnected(session) => {
                self.state.session = Some(*session);
                self.play();
            }
            MediaState(ref state)
                if state.current_time < self.config.duration.as_fractional_secs() =>
            {
                self.events.push(Event::SetElapsed {
                    elapsed: state.current_time,
                });
            }
            MediaState(_) if self.state.session.is_some() => {
                info!("Time limit reached. Advancing game");
                if let Some((cursor, track)) = self.load_next() {
                    self.state.session = None;
                    info!("Advancing to track {}", cursor);
                    self.events.push(Event::SetMedia {
                        media: media(&track, cursor),
                    });
                } else {
                    warn!("No more tracks. Shutting down");
                    self.events.push(Event::ClearMedia);
                    self.events.push(Event::Shutdown);
                    self.shutdown();
                }
            }
            MediaState(_) => {}
            event => warn!("Got unknown app event: {:?}", event),
        }
        if self.lifecycle == Lifecycle::Uninitialized {
            return vec![];
        }
        std::mem::replace(&mut self.events, vec![])
    }
}

fn media(track: &FsTrack, cursor: u64) -> Media {
    let cover = track.cover().map(|image| {
        let mime = image.mime;
        let bytes = base64::encode_config(&image.bytes, base64::URL_SAFE);
        Image {
            url: format!("data:{};base64,{}", mime, bytes),
            height: image.height,
            width: image.width,
        }
    });
    Media {
        id: track.id().to_owned(),
        cursor,
        artist: track.tags().and_then(|tag| tag.artist),
        title: track.tags().and_then(|tag| tag.title),
        cover,
    }
}

#[derive(Serialize, Debug)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
#[allow(dead_code)]
pub enum Event {
    ClearMedia,
    SetConfig {
        duration: f64,
    },
    SetElapsed {
        elapsed: f64,
    },
    SetMedia {
        media: Media,
    },
    #[serde(rename_all = "camelCase")]
    SetPlayback {
        is_playing: bool,
    },
    SetPlaylist {
        name: String,
    },
    Shutdown,
    TogglePlayback,
}

#[derive(Serialize, Debug)]
pub struct Media {
    id: String,
    cursor: u64,
    artist: Option<String>,
    title: Option<String>,
    cover: Option<Image>,
}

#[derive(Serialize, Debug)]
pub struct Image {
    url: String,
    height: u32,
    width: u32,
}

#[derive(Serialize, Debug)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Device {
    Cast {
        name: String,
        is_connected: bool,
        #[serde(skip_serializing)]
        connect: CastAddr,
    },
    Local {
        name: String,
        is_connected: bool,
    },
}

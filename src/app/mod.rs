use std::time::Duration;

use base64;
use floating_duration::TimeAsFloat;
use futures::sync::oneshot;

use backend::chromecast::{CastAddr, Device};
use cast::{MediaConnection, ReceiverConnection, Status};
use playlist::fs::{Playlist, Track};
use stream::{DrainListener, DrainTrigger};

pub struct AppState {
    playlist: Playlist,
    client: Option<Device>,
    connect: Option<ReceiverConnection>,
    session: Option<MediaConnection>,
    shutdown: Option<DrainTrigger>,
    devices: Vec<AppDevice>,
}

#[derive(Debug, Eq, PartialEq)]
pub enum AppLifecycle {
    Uninitialized,
    Loaded,
    Terminating,
}

pub struct AppConfig {
    pub duration: Duration,
    pub iterations: u64,
}

pub struct AppController {
    pub config: AppConfig,
    lifecycle: AppLifecycle,
    state: AppState,
    events: Vec<AppEvent>,
}

impl AppController {
    pub fn new(config: AppConfig, playlist: Playlist) -> (Self, DrainListener) {
        let (trigger, listener) = oneshot::channel();
        let state = AppState {
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
                lifecycle: AppLifecycle::Uninitialized,
                state,
                events,
            },
            listener,
        )
    }
}

impl AppController {
    pub fn set_devices(&mut self, devices: Vec<AppDevice>) {
        std::mem::replace(&mut self.state.devices, devices);
    }

    pub fn devices(&self) -> &[AppDevice] {
        &self.state.devices
    }

    pub fn set_client(&mut self, client: Device) {
        if let Some(mut old) = std::mem::replace(&mut self.state.client, Some(client)) {
            let _ = old.shutdown();
        }
    }

    pub fn playlist_name(&self) -> &str {
        self.state.playlist.name()
    }
}

// View lifecyle
impl AppController {
    pub fn view_did_load(&mut self) {
        self.lifecycle = AppLifecycle::Loaded;
    }

    pub fn view_lifecycle(&self) -> &AppLifecycle {
        &self.lifecycle
    }
}

// Playback controls
impl AppController {
    fn load_next(&mut self) -> Option<(u64, Track)> {
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
        self.lifecycle = AppLifecycle::Terminating;
    }
}

impl AppController {
    pub fn handle(&mut self, event: Status) -> Vec<AppEvent> {
        use cast::Status::*;
        if !self.events.is_empty() {
            debug!("AppEvents backlog of {} events", self.events.len());
        }
        match event {
            Connected(connect) => {
                self.state.connect = Some(*connect);
                if let Some((cursor, track)) = self.load_next() {
                    self.events.push(AppEvent::SetMedia {
                        media: media(track, cursor),
                    });
                    self.events.push(AppEvent::SetPlayback { is_playing: true });
                }
            }
            MediaConnected(session) => {
                self.state.session = Some(*session);
                self.play();
            }
            MediaStatus(ref status)
                if status.current_time < self.config.duration.as_fractional_secs() =>
            {
                self.events.push(AppEvent::SetElapsed {
                    elapsed: status.current_time,
                });
            }
            MediaStatus(_) if self.state.session.is_some() => {
                info!("Time limit reached. Advancing game");
                match self.load_next() {
                    Some((cursor, track)) => {
                        self.state.session = None;
                        info!("Advancing to track {}", cursor);
                        self.events.push(AppEvent::SetMedia {
                            media: media(track, cursor),
                        });
                    }
                    None => {
                        warn!("No more tracks. Shutting down");
                        self.events.push(AppEvent::ClearMedia);
                        self.events.push(AppEvent::Shutdown);
                        self.shutdown();
                    }
                }
            }
            MediaStatus(_) => {}
            event => warn!("Got unknown app event: {:?}", event),
        }
        if self.lifecycle == AppLifecycle::Uninitialized {
            return vec![];
        }
        std::mem::replace(&mut self.events, vec![])
    }
}

fn media(track: Track, cursor: u64) -> AppMedia {
    let cover = track.cover().map(|image| {
        let (width, height) = image
            .dimensions()
            .map(|(w, h, _)| (w, h))
            .unwrap_or_else(|| (600, 600));
        let mime = image.mime();
        let bytes = base64::encode_config(&image.unwrap(), base64::URL_SAFE);
        AppImage {
            url: format!("data:{};base64,{}", mime, bytes),
            height,
            width,
        }
    });
    AppMedia {
        id: track.id().to_owned(),
        cursor,
        artist: track.tags().and_then(|tag| tag.artist.to_option()),
        title: track.tags().and_then(|tag| tag.title.to_option()),
        cover,
    }
}

#[derive(Serialize, Debug)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
#[allow(dead_code)]
pub enum AppEvent {
    ClearMedia,
    SetConfig {
        duration: f64,
    },
    SetElapsed {
        elapsed: f64,
    },
    SetMedia {
        media: AppMedia,
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
pub struct AppMedia {
    id: String,
    cursor: u64,
    artist: Option<String>,
    title: Option<String>,
    cover: Option<AppImage>,
}

#[derive(Serialize, Debug)]
pub struct AppImage {
    url: String,
    height: u32,
    width: u32,
}

#[derive(Serialize, Debug)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AppDevice {
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

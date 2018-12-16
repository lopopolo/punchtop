use std::time::Duration;

use base64;
use floating_duration::TimeAsFloat;
use futures::sync::oneshot;

use backend::chromecast::Device;
use cast::{MediaConnection, ReceiverConnection, Status};
use playlist::fs::{Playlist, Track};
use stream::{DrainListener, DrainTrigger};

pub struct AppState {
    playlist: Playlist,
    client: Device,
    connect: Option<ReceiverConnection>,
    session: Option<MediaConnection>,
    shutdown: Option<DrainTrigger>,
    view_is_initialized: bool,
    is_shutting_down: bool,
}

pub struct AppConfig {
    pub duration: Duration,
    pub iterations: u64,
}

pub struct AppController {
    pub config: AppConfig,
    state: AppState,
    events: Vec<AppEvent>,
}

impl AppController {
    pub fn new(config: AppConfig, playlist: Playlist, client: Device) -> (Self, DrainListener) {
        let (trigger, listener) = oneshot::channel();
        let state = AppState {
            playlist,
            client,
            connect: None,
            session: None,
            shutdown: Some(trigger),
            view_is_initialized: false,
            is_shutting_down: false,
        };
        let events = vec![];
        (
            Self {
                config,
                state,
                events,
            },
            listener,
        )
    }
}

impl AppController {
    pub fn playlist_name(&self) -> &str {
        self.state.playlist.name()
    }

    pub fn signal_view_initialized(&mut self) {
        self.state.view_is_initialized = true;
    }

    pub fn is_shutting_down(&self) -> bool {
        self.state.is_shutting_down
    }

    fn load_next(&mut self) -> Option<(u64, Track)> {
        let connect = match self.state.connect {
            Some(ref connect) => connect,
            None => return None,
        };
        self.state.playlist.next().map(|(cursor, track)| {
            let _ = self.state.client.load(connect, &track);
            (cursor, track)
        })
    }

    pub fn pause(&self) {
        if let Some(ref session) = self.state.session {
            let _ = self.state.client.pause(session);
        };
    }

    pub fn play(&self) {
        if let Some(ref session) = self.state.session {
            let _ = self.state.client.play(session);
        };
    }

    fn shutdown(&mut self) {
        if let Some(ref session) = self.state.session {
            let _ = self.state.client.stop(session);
        }
        let _ = self.state.client.shutdown();
        if let Some(shutdown) = self.state.shutdown.take() {
            let _ = shutdown.send(());
        }
        self.state.is_shutting_down = true;
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
        if !self.state.view_is_initialized {
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

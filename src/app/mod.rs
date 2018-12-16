use std::time::Duration;

use base64;
use floating_duration::TimeAsFloat;
use futures::sync::oneshot;

use backend::chromecast::Device;
use cast::{MediaConnection, ReceiverConnection, Status};
use playlist::{Playlist, Track};
use stream::{DrainListener, DrainTrigger};

pub struct AppState {
    playlist: Playlist,
    client: Device,
    connect: Option<ReceiverConnection>,
    session: Option<MediaConnection>,
    shutdown: Option<DrainTrigger>,
}

pub struct AppConfig {
    pub duration: Duration,
    pub iterations: u64,
}

pub struct AppController {
    config: AppConfig,
    state: AppState,
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
        };
        (Self { config, state }, listener)
    }
}

impl AppController {
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
    }
}

impl AppController {
    pub fn handle(&mut self, event: Status) -> Option<AppEvent> {
        use cast::Status::*;
        match event {
            Connected(connect) => {
                self.state.connect = Some(*connect);
                self.load_next().map(|(_, track)| AppEvent::SetMedia {
                    media: media(track),
                })
            }
            MediaConnected(session) => {
                self.state.session = Some(*session);
                self.play();
                None
            }
            MediaStatus(status) => {
                if status.current_time < self.config.duration.as_fractional_secs() {
                    return Some(AppEvent::SetElapsed {
                        elapsed: status.current_time,
                    });
                }
                if self.state.session.is_none() {
                    return None;
                }
                info!("Time limit reached. Advancing game");
                match self.load_next() {
                    Some((cursor, track)) => {
                        self.state.session = None;
                        info!("Advancing to track {}", cursor);
                        Some(AppEvent::SetMedia {
                            media: media(track),
                        })
                    }
                    None => {
                        warn!("No more tracks. Shutting down");
                        self.shutdown();
                        Some(AppEvent::Shutdown)
                    }
                }
            }
            event => {
                warn!("Got unknown app event: {:?}", event);
                None
            }
        }
    }
}

fn media(track: Track) -> AppMedia {
    let cover = track.cover().map(|image| {
        let (width, height) = image
            .dimensions()
            .map(|(w, h, _)| (w, h))
            .unwrap_or_else(|| (600, 600));
        let mime = image.mime();
        let bytes = base64::encode_config(&image.unwrap(), base64::URL_SAFE);
        AppImage {
            url: format!("data:{};base64,{}", mime, bytes),
            height: height,
            width,
        }
    });
    AppMedia {
        id: track.id().to_owned(),
        artist: track.tags().and_then(|tag| tag.artist.to_option()),
        title: track.tags().and_then(|tag| tag.title.to_option()),
        cover,
    }
}

#[derive(Serialize, Debug)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
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
        initial: String,
    },
    Shutdown,
    TogglePlayback,
}

#[derive(Serialize, Debug)]
pub struct AppMedia {
    id: String,
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

use std::time::Duration;

use floating_duration::TimeAsFloat;
use futures::sync::oneshot;

use backend::chromecast::Device;
use cast::{MediaConnection, ReceiverConnection, Status};
use playlist::Playlist;
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

pub struct AppController{
    config: AppConfig,
    state: AppState
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
    fn load_next(&mut self) -> Option<u64> {
        let connect = match self.state.connect {
            Some(ref connect) => connect,
            None => return None,
        };
        self.state.playlist.next().map(|(cursor, track)| {
            let _ = self.state.client.load(connect, &track);
            cursor
        })
    }

    fn play(&self) {
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
    pub fn handle(&mut self, event: Status) -> Result<(), ()> {
        use cast::Status::*;
        match event {
            Connected(connect) => {
                self.state.connect = Some(*connect);
                self.load_next();
            }
            MediaConnected(session) => {
                self.state.session = Some(*session);
                self.play();
            }
            MediaStatus(status) => {
                if status.current_time < self.config.duration.as_fractional_secs() {
                    return Ok(());
                }
                if self.state.session.is_none() {
                    return Ok(());
                }
                info!("Time limit reached. Advancing game");
                match self.load_next() {
                    Some(cursor) => {
                        self.state.session = None;
                        info!("Advancing to track {}", cursor);
                    }
                    None => {
                        warn!("No more tracks. Shutting down");
                        self.shutdown();
                    }
                }
            }
            event => warn!("Got unknown app event: {:?}", event),
        };
        Ok(())
    }
}

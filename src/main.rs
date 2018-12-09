#![feature(inner_deref)]

extern crate byteorder;
extern crate bytes;
extern crate env_logger;
extern crate floating_duration;
extern crate futures;
extern crate hostname;
#[macro_use]
extern crate log;
extern crate mdns;
extern crate mp3_duration;
extern crate native_tls;
extern crate neguse_taglib;
extern crate neguse_types;
#[macro_use]
extern crate nom;
#[macro_use]
extern crate objc;
extern crate openssl;
extern crate protobuf;
extern crate rand;
extern crate rodio;
extern crate rouille;
extern crate rust_cast;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate tokio;
extern crate tokio_tls;
extern crate tree_magic;
extern crate url;
extern crate walkdir;

use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use tokio::prelude::*;
use tokio::runtime::Runtime;

use backend::PlayerKind;

mod backend;
mod cast;
mod playlist;

use cast::Status;

struct Game {
    playlist: playlist::Playlist,
    client: backend::chromecast::Device,
}

impl Game {
    fn session(&self) -> Option<String> {
        self.client
            .cast
            .as_ref()
            .and_then(|cast| cast.session.to_owned())
    }

    fn set_session(&mut self, session: String) {
        if let Some(ref mut cast) = self.client.cast {
            cast.session = Some(session);
        }
    }

    fn media_session(&self) -> Option<i32> {
        self.client
            .cast
            .as_ref()
            .and_then(|cast| cast.media_session)
    }

    fn set_media_session(&mut self, session: i32) {
        if let Some(ref mut cast) = self.client.cast {
            cast.media_session = Some(session);
        }
    }

    fn load_next(&mut self) -> Option<()> {
        match self.playlist.next() {
            Some(track) => {
                let _ = self.client.load(track);
                Some(())
            }
            None => None,
        }
    }

    fn play(&self) {
        let _ = self.client.play();
    }
}

fn main() {
    env_logger::init();
    let mut rt = Runtime::new().unwrap();
    let root = PathBuf::from("/Users/lopopolo/Downloads/test");
    let config = playlist::Config::new(Duration::new(5, 0), 10, root);
    let player = backend::chromecast::devices(config.clone())
        .filter(|p| p.kind() == PlayerKind::Chromecast)
        .find(|p| p.name() == "Kitchen Home");
    if let Some(mut backend) = player {
        let status = backend.connect(&mut rt).unwrap();
        let playlist = playlist::Playlist::from_directory(config);
        let mut game = Game {
            playlist,
            client: backend,
        };
        let play_loop = status
            .for_each(move |message| {
                info!("message: {:?}", message);
                info!("cast: {:?}", game.client.cast);
                match message {
                    Status::Connected(session_id) => match game.session() {
                        Some(_) => {}
                        None => {
                            info!("set session id: {}", session_id);
                            game.set_session(session_id);
                            game.load_next();
                        }
                    },
                    Status::MediaConnected(media_session_id) => match game.media_session() {
                        Some(_) => {}
                        None => {
                            game.set_media_session(media_session_id);
                            game.play();
                        }
                    },
                    message => warn!("Got unknown message: {:?}", message),
                };
                Ok(())
            })
            .into_future();
        rt.spawn(play_loop);
        thread::sleep(Duration::new(30, 0));
    }
    rt.shutdown_on_idle().wait().unwrap();
}

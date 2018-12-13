#![feature(inner_deref)]
#![feature(proc_macro_hygiene, decl_macro)]

extern crate byteorder;
extern crate bytes;
extern crate env_logger;
extern crate floating_duration;
extern crate futures;
extern crate futures_locks;
extern crate hostname;
#[macro_use]
extern crate log;
extern crate mdns;
extern crate mp3_duration;
extern crate mp4parse as mp4;
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
#[macro_use]
extern crate rocket;
extern crate rodio;
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
use std::time::Duration;

use floating_duration::TimeAsFloat;
use tokio::prelude::*;
use tokio::runtime::Runtime;

mod backend;
mod cast;
mod playlist;

use backend::chromecast::Device;
use cast::Status;
use playlist::Config;

struct Game {
    playlist: playlist::Playlist,
    client: backend::chromecast::Device,
    connect: Option<cast::ReceiverConnection>,
    session: Option<cast::MediaConnection>,
    config: Config,
}

impl Game {
    fn load_next(&mut self) -> Option<u64> {
        let connect = match self.connect {
            Some(ref connect) => connect,
            None => return None,
        };
        self.playlist.next().map(|(cursor, track)| {
            let _ = self.client.load(connect, &track);
            cursor
        })
    }

    fn play(&self) {
        if let Some(ref session) = self.session {
            let _ = self.client.play(session);
        };
    }

    fn shutdown(&mut self) {
        if let Some(ref session) = self.session {
            let _ = self.client.stop(session);
        }
        let _ = self.client.shutdown();
    }
}

const CAST: &str = "Kitchen Home";

fn main() {
    env_logger::init();
    let mut rt = Runtime::new().unwrap();
    let root = PathBuf::from("/Users/lopopolo/Downloads/test");
    let config = Config::new(Duration::new(60, 0), 20, root);
    let player = backend::chromecast::devices().find(|p| p.name == CAST);
    let player = match player {
        Some(player) => player,
        None => {
            eprintln!("Could not find chromecast named {}", CAST);
            ::std::process::exit(1);
        }
    };
    let playlist = playlist::Playlist::from_directory(&config);
    let (client, chan) = match Device::connect(player, playlist.registry(), &mut rt) {
        Ok(connect) => connect,
        Err(_) => {
            eprintln!("Could not connect to chromecast named {}", CAST);
            ::std::process::exit(1);
        }
    };
    let mut game = Game {
        playlist,
        client,
        connect: None,
        session: None,
        config,
    };
    let play_loop = chan
        .for_each(move |message| {
            match message {
                Status::Connected(connect) => {
                    game.connect = Some(*connect);
                    game.load_next();
                }
                Status::MediaConnected(session) => {
                    game.session = Some(*session);
                    game.play();
                }
                Status::MediaStatus(status) => {
                    let advance = status.current_time > game.config.duration.as_fractional_secs()
                        && game.session.is_some();
                    if advance {
                        info!("Time limit reached. Advancing game");
                        match game.load_next() {
                            Some(cursor) => {
                                game.session = None;
                                info!("Advancing to track {}", cursor);
                            }
                            None => {
                                warn!("No more tracks. Shutting down");
                                game.shutdown();
                            }
                        }
                    }
                }
                message => warn!("Got unknown message: {:?}", message),
            };
            Ok(())
        })
        .into_future();
    rt.spawn(play_loop);
    rt.shutdown_on_idle().wait().unwrap();
}

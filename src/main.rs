#![feature(inner_deref)]

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
    connect: Option<cast::ReceiverConnection>,
    media_connect: Option<cast::MediaConnection>,
}

impl Game {
    fn load_next(&mut self) -> Option<()> {
        let connect = match self.connect {
            Some(ref connect) => connect,
            None => return None,
        };
        self.playlist.next().map(|track| {
            let _ = self.client.load(connect, track);
        })
    }

    fn play(&self) {
        if let Some(ref connect) = self.media_connect {
            let _ = self.client.play(connect);
        };
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
            connect: None,
            media_connect: None,
        };
        let play_loop = status
            .for_each(move |message| {
                match message {
                    Status::Connected(connect) => {
                        game.connect = Some(connect);
                        game.load_next();
                    },
                    Status::MediaConnected(connect) => {
                        game.media_connect = Some(connect.clone());
                        if connect.session.is_none() {
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

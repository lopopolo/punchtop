#![feature(inner_deref)]
#![feature(proc_macro_hygiene, decl_macro)]

extern crate base64;
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

use futures::prelude::*;
use futures::Stream;
use tokio::runtime::Runtime;

mod app;
mod backend;
mod cast;
mod playlist;
mod stream;

use app::{AppConfig, AppController};
use backend::chromecast::Device;
use stream::drain;

const CAST: &str = "Kitchen Home";

fn main() {
    env_logger::init();
    let mut rt = Runtime::new().unwrap();
    let root = PathBuf::from("/Users/lopopolo/Downloads/test");
    let config = AppConfig { duration: Duration::new(10, 0), iterations: 3 };
    let player = backend::chromecast::devices().find(|p| p.name == CAST);
    let player = match player {
        Some(player) => player,
        None => {
            eprintln!("Could not find chromecast named {}", CAST);
            ::std::process::exit(1);
        }
    };
    let playlist = playlist::Playlist::from_directory(&root, &config);
    let (client, chan) = match Device::connect(player, playlist.registry(), &mut rt) {
        Ok(connect) => connect,
        Err(err) => {
            warn!("chromecast connect error: {:?}", err);
            eprintln!("Could not connect to chromecast named {}", CAST);
            ::std::process::exit(1);
        }
    };
    let (mut controller, shutdown) = AppController::new(config, playlist, client);
    let play_loop = drain(chan, shutdown.map_err(|_| ()))
        .for_each(move |event| controller.handle(event))
        .into_future();
    rt.spawn(play_loop);
    rt.shutdown_on_idle().wait().unwrap();
}

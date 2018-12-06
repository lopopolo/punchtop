#![feature(inner_deref)]

extern crate byteorder;
extern crate bytes;
extern crate floating_duration;
extern crate futures;
extern crate hostname;
// #[macro_use]
// extern crate log;
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
#[macro_use]
extern crate serde_json;
extern crate tokio;
extern crate tokio_codec;
extern crate tokio_io;
extern crate tokio_tls;
extern crate tree_magic;
extern crate url;
extern crate walkdir;

use backend::PlayerKind;
use std::path::PathBuf;
use std::time::Duration;

mod backend;
mod cast;
mod playlist;

fn main() {
    let root = PathBuf::from("/Users/lopopolo/Downloads/test");
    let config = playlist::Config::new(Duration::new(5, 0), 10, root);
    let player = backend::players(config.clone())
        .filter(|p| p.kind() == PlayerKind::Chromecast)
        .find(|p| p.name() == "TV");
    if let Some(mut backend) = player {
        let playlist = playlist::Playlist::from_directory(config);

        match backend.connect() {
            Ok(_) => {
                for track in playlist {
                    println!("{:?}", track);
                    if let Err(err) = backend.play(track) {
                        println!("Error during playback: {:?}", err);
                        continue;
                    }
                }
            }
            Err(err) => println!("Error when connecting: {:?}", err),
        }
        let _ = backend.close();
    }
}

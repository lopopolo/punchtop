#![feature(inner_deref)]

#[macro_use]
extern crate crossbeam_channel;
extern crate floating_duration;
extern crate hostname;
extern crate interfaces;
// #[macro_use]
// extern crate log;
extern crate mdns;
extern crate mp3_duration;
extern crate neguse_taglib;
extern crate neguse_types;
#[macro_use]
extern crate nom;
#[macro_use]
extern crate objc;
extern crate rand;
extern crate rodio;
extern crate rouille;
extern crate rust_cast;
extern crate tree_magic;
extern crate walkdir;

use backend::PlayerKind;
use std::path::PathBuf;
use std::time::Duration;

mod backend;
mod playlist;

fn main() {
    let root = PathBuf::from("/Users/lopopolo/Downloads/test");
    let config = playlist::Config::new(Duration::new(5, 0), 10, root);
    let player = backend::players(config.clone())
        .filter(|p| p.kind() == PlayerKind::Chromecast)
        .find(|p| p.name() == "TV");
    if let Some(mut backend) = player {
        let playlist = playlist::Playlist::from_directory(config);

        if backend.connect().is_ok() {
            for track in playlist {
                println!("{:?}", track);
                if let Err(err) = backend.play(track) {
                    println!("Error during playback: {:?}", err);
                    continue;
                }
            }
        }
        let _ = backend.close();
    }
}

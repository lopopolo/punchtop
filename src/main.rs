#![feature(inner_deref)]

extern crate hostname;
#[macro_use]
extern crate lazy_static;
extern crate mdns;
extern crate mp3_duration;
#[macro_use]
extern crate nom;
extern crate rand;
extern crate rodio;
extern crate rust_cast;
extern crate taglib;
extern crate walkdir;

mod backend;
mod playlist;

use backend::{Device, Player};
use std::path::Path;
use std::time::Duration;

fn main() {
    let mut backend = backend::devices()
        .map(|b| {
            println!("{}", b.name());
            b
        })
        .find(|b| match b {
            Device::Local(_) => true,
            _ => false,
        })
        .unwrap();
    backend.connect();

    let config = playlist::Config::new(Duration::new(5, 0), 10);
    let playlist =
        playlist::Playlist::from_directory(Path::new("/Users/lopopolo/Downloads/test"), config);
    for track in playlist {
        match (
            track.metadata.artist(),
            track.metadata.album(),
            track.metadata.title(),
        ) {
            (Some(artist), Some(album), Some(title)) => {
                println!("{}", title);
                println!("{} -- {}", artist, album);
            }
            (Some(artist), None, Some(title)) => {
                println!("{}", title);
                println!("{}", artist);
            }
            (None, None, Some(title)) => {
                println!("{}", title);
            }
            _ => (),
        }
        if backend.play(&track.path, track.duration).is_err() {
            continue;
        }
        for device in backend::devices() {
            println!("{}", device.name());
        }
    }
}

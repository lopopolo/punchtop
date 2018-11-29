#![feature(inner_deref)]

extern crate mdns;
extern crate mp3_duration;
#[macro_use]
extern crate nom;
extern crate rand;
extern crate rodio;
extern crate taglib;
extern crate walkdir;

mod backend;
mod playlist;

use backend::{local,BackendDevice};
use std::path::Path;
use std::time::Duration;

fn main() {
    let backend = local::BackendDevice::new();

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
        if let Ok(ref sink) = backend {
            match sink.play(&track.path, track.duration) {
                Err(_) => continue,
                _ => (),
            };
        }
    }
}

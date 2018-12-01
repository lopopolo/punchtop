#![feature(inner_deref)]

extern crate floating_duration;
extern crate hostname;
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

use backend::PlayerKind;
use std::path::Path;
use std::time::Duration;

fn main() {
    let player = backend::players()
        .filter(|p| p.kind() == PlayerKind::Chromecast)
        .find(|p| p.name() == "Soundbar");
    if let Some(mut backend) = player {
        backend.connect().ok().unwrap();
        let config = playlist::Config::new(Duration::new(5, 0), 10);
        let playlist =
            playlist::Playlist::from_directory(Path::new("/Users/lopopolo/Downloads/test"), config);
        for track in playlist {
            let metadata: Vec<&str> = vec![track.metadata.artist(), track.metadata.title(), track.metadata.album()]
                .iter()
                .filter(|md| md.is_some())
                .map(|md| md.unwrap())
                .collect();
            println!("{}", metadata.join(" -- "));
            if let Err(err) = backend.play(&track.path, track.duration) {
                println!("Error during playback: {:?}", err);
                continue;
            }
        }
    }
}

#![feature(inner_deref)]

extern crate floating_duration;
extern crate hostname;
extern crate mdns;
extern crate mp3_duration;
#[macro_use]
extern crate nom;
extern crate rand;
extern crate rodio;
extern crate rouille;
extern crate rust_cast;
extern crate taglib;
extern crate tree_magic;
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
        let config = playlist::Config::new(Duration::new(5, 0), 10);
        let root = Path::new("/Users/lopopolo/Downloads/test");
        let playlist = playlist::Playlist::from_directory(root, config);

        backend.connect(root).ok().unwrap();

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

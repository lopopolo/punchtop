#![feature(inner_deref)]

extern crate mp3_duration;
extern crate rand;
extern crate rodio;
extern crate taglib;
extern crate walkdir;

mod playlist;

use std::path::Path;
use std::time::Duration;

fn main() {
    let device = rodio::default_output_device().unwrap();
    let sink = rodio::Sink::new(&device);

    let config = playlist::Config::new(Duration::new(5, 0), 10);
    let playlist =
        playlist::Playlist::from_directory(Path::new("/Users/lopopolo/Downloads/test"), config);
    for track in playlist {
        match (track.metadata.artist(), track.metadata.album(), track.metadata.title()) {
            (Some(artist), Some(album), Some(title)) => {
                println!("{}", title);
                println!("{} -- {}", artist, album);
            },
            (Some(artist), None, Some(title)) => {
                println!("{}", title);
                println!("{}", artist);
            },
            (None, None, Some(title)) => {
                println!("{}", title);
            },
            _ => (),
        }
        sink.append(track.stream());
        sink.sleep_until_end();
    }
}

extern crate rodio;

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
        sink.append(track);
        sink.sleep_until_end();
    }
}

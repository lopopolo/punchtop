extern crate rodio;
extern crate walkdir;

use rodio::Source;
use std::ffi::OsStr;
use std::io::BufReader;
use std::path::Path;
use std::time::Duration;
use walkdir::{DirEntry, WalkDir};

fn is_music(entry: &DirEntry) -> bool {
    let extension = entry
        .path()
        .file_name()
        .and_then(|p| Path::new(p).extension())
        .and_then(OsStr::to_str);
    match extension {
        Some("mp3") => true,
        _ => false,
    }
}

fn main() {
    let device = rodio::default_output_device().unwrap();
    let sink = rodio::Sink::new(&device);

    let walker = WalkDir::new("/Users/lopopolo/Downloads/test")
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| is_music(e));
    for entry in walker {
        println!("{}", entry.path().display());
        let file = std::fs::File::open(entry.path()).unwrap();
        let source = rodio::Decoder::new(BufReader::new(file)).unwrap();
        let source = source.take_duration(Duration::from_secs(60));
        sink.append(source);

        sink.sleep_until_end();
    }
}

extern crate mp3_duration;
extern crate rand;
extern crate walkdir;

use self::rand::Rng;
use self::walkdir::{DirEntry, WalkDir};
use rodio::{decoder, source, Source};
use std::collections::VecDeque;
use std::ffi::OsStr;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::Duration;
use std::vec::Vec;

fn is_music(entry: &DirEntry) -> bool {
    let extension = entry
        .path()
        .file_name()
        .and_then(|p| Path::new(p).extension())
        .and_then(OsStr::to_str);
    match extension {
        Some(ext) if ext == "mp3" => true,
        _ => false,
    }
}

pub struct Config {
    duration: Duration,
    count: u64,
}

impl Config {
    pub fn new(duration: Duration, count: u64) -> Self {
        Config { duration, count }
    }
}

pub struct Playlist {
    tracks: VecDeque<PathBuf>,
    config: Config,
    cursor: u64,
}

impl Playlist {
    pub fn from_directory(dir: &Path, config: Config) -> Self {
        let mut vec = Vec::new();
        let track_duration = config.duration;
        let walker = WalkDir::new(dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| is_music(e))
            .filter(|e| {
                mp3_duration::from_path(e.path())
                    .ok()
                    .and_then(|duration| duration.checked_sub(track_duration))
                    .map_or(false, |_| true)
            });
        for entry in walker {
            vec.push(PathBuf::from(entry.path()));
        }
        rand::thread_rng().shuffle(&mut vec);

        Playlist {
            tracks: VecDeque::from(vec),
            config,
            cursor: 0,
        }
    }
}

impl Iterator for Playlist {
    type Item = source::TakeDuration<source::Buffered<decoder::Decoder<BufReader<File>>>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.cursor >= self.config.count {
            return None;
        }
        if let Some(p) = self.tracks.pop_front() {
            File::open(p.as_os_str())
                .ok()
                .and_then(|f| rodio::Decoder::new(BufReader::new(f)).ok())
                .map(|s| s.buffered())
                .map(|s| s.take_duration(self.config.duration))
                .map(|s| {
                    self.cursor += 1;
                    self.tracks.push_back(p);
                    s
                })
        } else {
            None
        }
    }
}

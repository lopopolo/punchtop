use std::collections::VecDeque;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::time::Duration;
use std::vec::Vec;

use rand::seq::SliceRandom;
use rand::thread_rng;
use walkdir::{DirEntry, WalkDir};

fn direntry_to_extension(entry: &DirEntry) -> Option<&str> {
    Path::new(entry.path()).extension().and_then(OsStr::to_str)
}

fn is_music(entry: &DirEntry) -> bool {
    match direntry_to_extension(entry) {
        Some(ext) if ext == "mp3" => true,
        Some(ext) if ext == "m4a" => true,
        _ => false,
    }
}

#[derive(Debug)]
pub struct Config {
    duration: Duration,
    count: u64,
}

impl Config {
    pub fn new(duration: Duration, count: u64) -> Self {
        Config { duration, count }
    }
}

#[derive(Debug)]
pub struct Track {
    pub path: PathBuf,
    pub duration: Duration,
}

#[derive(Debug)]
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
            .filter(is_music)
            .filter(|e| match direntry_to_extension(e) {
                Some("mp3") => mp3_duration::from_path(e.path())
                    .ok()
                    .and_then(|duration| duration.checked_sub(track_duration))
                    .is_some(),
                _ => true,
            });
        for entry in walker {
            vec.push(PathBuf::from(entry.path()));
        }

        let mut rng = thread_rng();
        vec.shuffle(&mut rng);

        Playlist {
            tracks: VecDeque::from(vec),
            config,
            cursor: 0,
        }
    }
}

impl Iterator for Playlist {
    type Item = Track;

    fn next(&mut self) -> Option<Self::Item> {
        if self.cursor >= self.config.count {
            return None;
        }
        self.cursor += 1;
        match self.tracks.pop_front() {
            Some(path) => {
                let track = Track {
                    path: path.to_path_buf(),
                    duration: self.config.duration,
                };
                self.tracks.push_back(path);
                Some(track)
            }
            None => None,
        }
    }
}

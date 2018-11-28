use rand::seq::SliceRandom;
use rand::thread_rng;
use rodio::{decoder, source, Source};
use std::collections::VecDeque;
use std::ffi::OsStr;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::Duration;
use std::vec::Vec;
use walkdir::{DirEntry, WalkDir};

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

pub struct Metadata {
    pub artist: Option<String>,
    pub album: Option<String>,
    pub title: Option<String>,
}

impl Metadata {
    fn new(path: &Path) -> Self {
        match taglib::File::new(path) {
            Ok(reader) => {
                let tag = reader.tag().ok();
                let artist = tag.as_ref().and_then(|t| t.artist());
                let album = tag.as_ref().and_then(|t| t.album());
                let title = tag.as_ref().and_then(|t| t.title());
                Metadata {
                    artist,
                    album,
                    title,
                }
            }
            Err(_) => Metadata {
                artist: None,
                album: None,
                title: None,
            },
        }
    }

    pub fn artist(&self) -> Option<&str> {
        self.artist.deref()
    }

    pub fn album(&self) -> Option<&str> {
        self.album.deref()
    }

    pub fn title(&self) -> Option<&str> {
        self.title.deref()
    }
}

pub struct Track {
    path: PathBuf,
    duration: Duration,
    pub metadata: Metadata,
}

impl Track {
    pub fn stream(
        self,
    ) -> source::TakeDuration<source::Buffered<decoder::Decoder<BufReader<File>>>> {
        File::open(self.path.as_os_str())
            .ok()
            .and_then(|f| rodio::Decoder::new(BufReader::new(f)).ok())
            .map(|s| s.buffered())
            .map(|s| s.take_duration(self.duration))
            .unwrap()
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
        if let Some(p) = self.tracks.pop_front() {
            println!("{:?}", p);
            Some(Track {
                path: p.to_path_buf(),
                duration: self.config.duration,
                metadata: Metadata::new(&p),
            })
        } else {
            None
        }
    }
}

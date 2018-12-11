use std::collections::VecDeque;
use std::fs::File;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::time::Duration;
use std::vec::Vec;

use floating_duration::TimeAsFloat;
use neguse_taglib::{get_front_cover, get_tags};
use neguse_types::{Image, Tags};
use rand::seq::SliceRandom;
use rand::thread_rng;
use walkdir::WalkDir;

// https://developers.google.com/cast/docs/media#audio_codecs
fn is_audio_media(path: &Path) -> bool {
    let mime: &str = &tree_magic::from_filepath(path);
    match mime {
        "audio/mpeg" | "audio/mp3" => true,
        "audio/aac" | "audio/mp4" => true,
        "audio/flac" => true,
        "audio/ogg" | "application/ogg" => true,
        "audio/webm" => true,
        _ => false,
    }
}

fn is_sufficient_duration(path: &Path, required_duration: Duration) -> bool {
    let mime: &str = &tree_magic::from_filepath(path);
    match mime {
        "audio/mpeg" | "audio/mp3" => mp3_duration::from_path(path)
            .ok()
            .and_then(|duration| duration.checked_sub(required_duration))
            .is_some(),
        "audio/aac" | "audio/mp4" => {
            let mut fd = match File::open(path) {
                Ok(fd) => fd,
                Err(_) => return false,
            };
            let mut buf = Vec::new();
            if fd.read_to_end(&mut buf).is_err() {
                return false;
            }
            let mut c = Cursor::new(&buf);
            let mut context = mp4::MediaContext::new();
            if mp4::read_mp4(&mut c, &mut context).is_err() {
                return false;
            }
            let mut valid = true;
            for track in context.tracks {
                let mut track_valid = false;
                if let (Some(duration), Some(timescale)) = (track.duration, track.timescale) {
                    if timescale.0 > 0 {
                        let duration = duration.0 as f64 / timescale.0 as f64;
                        track_valid = duration >= required_duration.as_fractional_secs();
                    }
                }
                valid = valid && track_valid;
            }
            valid
        }
        _ => false,
    }
}

#[derive(Clone, Debug)]
pub struct Config {
    pub duration: Duration,
    pub count: u64,
    root: PathBuf,
}

impl Config {
    pub fn new(duration: Duration, count: u64, root: PathBuf) -> Self {
        Config {
            duration,
            count,
            root,
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}

#[derive(Clone, Debug)]
pub struct Track {
    path: PathBuf,
}

impl Track {
    pub fn new(path: PathBuf) -> Self {
        Track { path }
    }

    pub fn tags(&self) -> Option<Tags> {
        get_tags(&self.path).ok()
    }

    pub fn cover(&self) -> Option<Image> {
        get_front_cover(&self.path).ok().filter(|img| img.is_some())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn content_type(&self) -> String {
        tree_magic::from_filepath(self.path())
    }
}

#[derive(Debug)]
pub struct Playlist {
    tracks: VecDeque<PathBuf>,
    config: Config,
    cursor: u64,
}

impl Playlist {
    pub fn from_directory(config: &Config) -> Self {
        let mut vec = Vec::new();
        let walker = WalkDir::new(config.root())
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| is_audio_media(e.path()))
            .filter(|e| is_sufficient_duration(e.path(), config.duration));
        for entry in walker {
            vec.push(PathBuf::from(entry.path()));
        }

        let mut rng = thread_rng();
        vec.shuffle(&mut rng);

        Playlist {
            tracks: VecDeque::from(vec),
            config: config.clone(),
            cursor: 0,
        }
    }
}

impl Iterator for Playlist {
    type Item = (u64, Track);

    fn next(&mut self) -> Option<Self::Item> {
        if self.cursor >= self.config.count {
            return None;
        }
        self.cursor += 1;
        match self.tracks.pop_front() {
            Some(path) => {
                let track = Track {
                    path: path.to_path_buf(),
                };
                self.tracks.push_back(path);
                Some((self.cursor, track))
            }
            None => None,
        }
    }
}

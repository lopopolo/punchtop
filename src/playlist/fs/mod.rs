use std::collections::{HashMap, VecDeque};
use std::fs::File;
use std::io::{Cursor, Read};
use std::iter;
use std::panic::catch_unwind;
use std::path::{Path, PathBuf};
use std::time::Duration;
use std::vec::Vec;

use floating_duration::TimeAsFloat;
use neguse_taglib::{get_front_cover, get_tags};
use neguse_types::{Image, Tags};
use rand::distributions::Alphanumeric;
use rand::seq::SliceRandom;
use rand::{thread_rng, Rng};
use walkdir::WalkDir;

use app::AppConfig;

pub mod dir;
pub mod music;

pub fn new(root: &Path, name: &str, config: &AppConfig) -> Playlist {
    let mut vec = Vec::new();
    let walker = WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file());

    for entry in walker {
        vec.push(PathBuf::from(entry.path()));
    }

    let mut rng = thread_rng();
    vec.shuffle(&mut rng);

    let vec: Vec<Track> = vec
        .into_iter()
        .take(2 * config.iterations as usize)
        .filter(|path| is_audio_media(&path))
        .filter(|path| is_sufficient_duration(&path, config.duration))
        .map(Track::new)
        .collect();

    Playlist {
        name: name.to_owned(),
        tracks: VecDeque::from(vec),
        iterations: config.iterations,
        cursor: 0,
    }
}

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
        "audio/mpeg" | "audio/mp3" => {
            let ok = catch_unwind(|| {
                mp3_duration::from_path(path)
                    .ok()
                    .and_then(|duration| duration.checked_sub(required_duration))
                    .is_some()
            });
            match ok {
                Ok(ok) => ok,
                Err(_) => false,
            }
        }
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
pub struct Track {
    path: PathBuf,
    id: String,
}

impl Track {
    pub fn new(path: PathBuf) -> Self {
        let mut rng = thread_rng();
        let id: String = iter::repeat(())
            .map(|()| rng.sample(Alphanumeric))
            .take(8)
            .collect();
        Track { path, id }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn tags(&self) -> Option<Tags> {
        get_tags(&self.path).ok()
    }

    pub fn cover(&self) -> Option<Image> {
        get_front_cover(&self.path).ok().filter(|img| img.is_some())
    }

    pub fn stream(&self) -> Option<impl Read> {
        File::open(&self.path).ok()
    }

    pub fn content_type(&self) -> String {
        tree_magic::from_filepath(&self.path)
    }
}

#[derive(Debug)]
pub struct Playlist {
    name: String,
    tracks: VecDeque<Track>,
    iterations: u64,
    cursor: u64,
}

impl Playlist {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn registry(&self) -> HashMap<String, Track> {
        let mut registry = HashMap::new();
        for track in &self.tracks {
            registry.insert(track.id().to_owned(), track.clone());
        }
        registry
    }
}

impl Iterator for Playlist {
    type Item = (u64, Track);

    fn next(&mut self) -> Option<Self::Item> {
        if self.cursor >= self.iterations {
            return None;
        }
        self.cursor += 1;
        let track = self.tracks.pop_front()?;
        self.tracks.push_back(track.clone());
        Some((self.cursor, track))
    }
}
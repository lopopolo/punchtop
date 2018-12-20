use std::collections::{HashMap, VecDeque};
use std::convert::TryInto;
use std::fs::File;
use std::io::{Cursor, Read};
use std::iter;
use std::panic::catch_unwind;
use std::path::{Path, PathBuf};
use std::time::Duration;
use std::vec::Vec;

use mp4parse::{MediaContext, TrackScaledTime, TrackTimeScale};
use neguse_taglib::{get_front_cover, get_tags};
use neguse_types::{Image, Tags};
use rand::distributions::Alphanumeric;
use rand::seq::SliceRandom;
use rand::{thread_rng, Rng};
use walkdir::WalkDir;

use crate::app::Config;

pub mod dir;
pub mod music;

const FALLBACK_PLAYLIST_SIZE: usize = 60;

pub fn new(root: &Path, name: &str, config: &Config) -> Playlist {
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

    let iterations = config
        .iterations
        .try_into()
        .unwrap_or(FALLBACK_PLAYLIST_SIZE);
    let playlist: Vec<Track> = vec
        .into_iter()
        .filter_map(|path| {
            if is_audio_media(&path) && is_sufficient_duration(&path, config.duration) {
                Some(Track::new(path))
            } else {
                None
            }
        })
        .take(iterations)
        .collect();

    Playlist {
        name: name.to_owned(),
        tracks: VecDeque::from(playlist),
        iterations: config.iterations,
        cursor: 0,
    }
}

// https://developers.google.com/cast/docs/media#audio_codecs
fn is_audio_media(path: &Path) -> bool {
    let mime: &str = &tree_magic::from_filepath(path);
    match mime {
        "audio/mpeg" | "audio/mp3" | "audio/aac" | "audio/mp4" | "audio/flac" | "audio/ogg"
        | "application/ogg" | "audio/webm" => true,
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
            if let Ok(ok) = ok {
                ok
            } else {
                warn!(
                    "Panic when checking duration of {} filetype at {:?}",
                    mime, path
                );
                false
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
            let mut context = MediaContext::new();
            if mp4parse::read_mp4(&mut c, &mut context).is_err() {
                return false;
            }
            context.tracks.into_iter().all(|track| {
                match scale_to_micros(track.duration, track.timescale) {
                    Some(duration) if duration > required_duration.as_micros() => true,
                    _ => false,
                }
            })
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
        let id = iter::repeat(())
            .map(|()| rng.sample(Alphanumeric))
            .take(8)
            .collect();
        Self { path, id }
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

fn scale_to_micros(
    duration: Option<TrackScaledTime<u64>>,
    scale: Option<TrackTimeScale<u64>>,
) -> Option<u128> {
    let microseconds_per_second = 1_000_000;
    let numerator = duration.map(|d| d.0)?;
    let denominator = scale.map(|s| s.0)?;

    if denominator == 0 {
        return None;
    }

    let integer = numerator / denominator;
    let remainder = numerator % denominator;
    let integer = integer.checked_mul(microseconds_per_second)?;
    let remainder = remainder.checked_mul(microseconds_per_second)?;
    (remainder / denominator)
        .checked_add(integer)
        .map(u128::from)
}

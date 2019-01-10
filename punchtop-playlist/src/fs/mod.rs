use std::collections::{HashMap, VecDeque};
use std::convert::TryInto;
use std::fs::File;
use std::io::{Cursor, Read};
use std::iter;
use std::panic;
use std::path::{Path, PathBuf};
use std::time::Duration;
use std::vec::Vec;

use mp4parse::{MediaContext, TrackScaledTime, TrackTimeScale};
use punchtop_audio::{self, Image, Tags};
use rand::distributions::Alphanumeric;
use rand::seq::SliceRandom;
use rand::{self, Rng};
use walkdir::WalkDir;

pub mod dir;
pub mod music;

const FALLBACK_PLAYLIST_SIZE: usize = 60;

pub fn playlist(root: &Path, name: &str, duration: Duration, iterations: u64) -> Playlist {
    let mut vec = Vec::new();
    let walker = WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file());

    for entry in walker {
        vec.push(PathBuf::from(entry.path()));
    }

    let mut rng = rand::thread_rng();
    vec.shuffle(&mut rng);

    let playlist: Vec<_> = vec
        .into_iter()
        .filter_map(|path| {
            if is_audio_media(&path) && is_sufficient_duration(&path, duration) {
                Some(Track::new(path, duration))
            } else {
                None
            }
        })
        .take(iterations.try_into().unwrap_or(FALLBACK_PLAYLIST_SIZE))
        .collect();

    Playlist {
        name: name.to_owned(),
        tracks: VecDeque::from(playlist),
        iterations,
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
            let ok = panic::catch_unwind(|| {
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
    duration: Duration,
}

impl Track {
    pub fn new(path: PathBuf, duration: Duration) -> Self {
        let mut rng = rand::thread_rng();
        let id = iter::repeat(())
            .map(|()| rng.sample(Alphanumeric))
            .take(8)
            .collect();
        Self { path, id, duration }
    }
}

impl punchtop_audio::Track for Track {
    fn id(&self) -> &str {
        &self.id
    }

    fn duration(&self) -> Duration {
        self.duration
    }

    fn tags(&self) -> Option<Tags> {
        let tags = neguse_taglib::get_tags(&self.path).ok()?;
        Some(Tags {
            artist: tags.artist.to_option(),
            title: tags.title.to_option(),
            album: tags.album.to_option(),
        })
    }

    fn cover(&self) -> Option<Image> {
        let cover = neguse_taglib::get_front_cover(&self.path)
            .ok()
            .filter(|img| img.is_some())?;
        let mime = cover.mime();
        let (width, height, _) = cover.dimensions()?;
        Some(Image {
            bytes: cover.unwrap(),
            mime,
            width,
            height,
        })
    }

    fn stream(&self) -> Option<Box<dyn Read>> {
        let file = File::open(&self.path).ok()?;
        Some(Box::new(file) as Box<dyn Read>)
    }

    fn content_type(&self) -> String {
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

    pub fn registry(&self) -> HashMap<String, Box<dyn punchtop_audio::Track + Send + Sync>> {
        let mut registry = HashMap::new();
        for track in &self.tracks {
            let track: Box<dyn punchtop_audio::Track + Send + Sync> = Box::new(track.clone());
            registry.insert(track.id().to_owned(), track);
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

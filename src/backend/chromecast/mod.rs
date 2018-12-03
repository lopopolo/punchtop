use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crossbeam_channel::{after, unbounded, Receiver, Sender};
use floating_duration::TimeAsFloat;
use mdns::RecordKind;
use rust_cast::channels::media::{Image, Media, Metadata, MusicTrackMediaMetadata, StreamType};

use backend::{self, Error, Player, PlayerKind};
use playlist::{Config, Track};

mod media_server;
mod parser;
mod worker;
use self::worker::{CastResult, Control};

/// Google Chromecast multicast service identifier.
const SERVICE_NAME: &str = "_googlecast._tcp.local";
/// Key in DNS TXT record for Chromecast "friendly name".
const CHROMECAST_NAME_KEY: &str = "fn";
/// Timeout for discovering Chromecast devices with mdns.
const DISCOVER_TIMEOUT: Duration = Duration::from_millis(1000);
/// Timeout for communication with Chromecast control thread.
const RECV_TIMEOUT: Duration = Duration::from_millis(150);

/// Configuration for Chromecast endpoints.
struct CastAddr {
    /// Name of a Chromecast as given by the `fn` field in its DNS TXT record.
    name: String,
    /// Address of Chromecast as discovered by mdns.
    addr: SocketAddr,
}

impl CastAddr {
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl PartialEq for CastAddr {
    fn eq(&self, other: &CastAddr) -> bool {
        self.name == other.name
    }
}

impl Eq for CastAddr {}

impl Hash for CastAddr {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

struct Channel {
    tx: Sender<Control>,
    rx: Receiver<CastResult>,
}

struct CastTrack<'a> {
    root: &'a Path,
    server: SocketAddr,
    track: Track,
}

impl<'a> CastTrack<'a> {
    fn url_path(&self) -> Option<String> {
        PathBuf::from(self.track.path())
            .strip_prefix(self.root)
            .ok()
            .and_then(|suffix| suffix.to_str())
            .map(String::from)
    }

    pub fn media(&self) -> Option<String> {
        self.url_path()
            .map(|path| format!("http://{}/media/{}", self.server, path))
    }

    pub fn image(&self) -> Option<String> {
        self.url_path()
            .map(|path| format!("http://{}/image/{}", self.server, path))
    }

    pub fn metadata(&self) -> Option<MusicTrackMediaMetadata> {
        let images = self
            .image()
            .map(|image| {
                vec![Image {
                    url: image,
                    dimensions: self
                        .track
                        .cover()
                        .and_then(|img| img.dimensions().map(|(w, h, _)| (w, h))),
                }]
            })
            .unwrap_or_else(|| vec![]);
        self.track.tags().map(|tags| {
            MusicTrackMediaMetadata {
                album_name: tags.album.to_option(),
                title: tags.title.to_option(),
                album_artist: tags.album_artist.to_option(),
                artist: tags.artist.to_option(),
                composer: tags.composer.to_option(),
                track_number: Some(1 as u32), // use game cursor
                disc_number: Some(1),
                release_date: tags.date.to_option().map(|d| d.to_iso_8601()),
                images,
            }
        })
    }
}

pub struct Device {
    game_config: Config,
    connect_config: CastAddr,
    chan: Option<Channel>,
    media_server_bind_addr: Option<SocketAddr>,
}

impl Player for Device {
    fn name(&self) -> String {
        self.connect_config.name().to_owned()
    }

    fn kind(&self) -> PlayerKind {
        PlayerKind::Chromecast
    }

    fn connect(&mut self) -> backend::Result {
        match media_server::spawn(self.game_config.root(), self.connect_config.addr) {
            Ok(addr) => {
                self.media_server_bind_addr = Some(addr);

                let (control_tx, control_rx) = unbounded();
                let (status_tx, status_rx) = unbounded();
                let cast_addr = self.connect_config.addr.to_owned();
                worker::spawn(cast_addr, worker::chan(status_tx, control_rx));

                let res = select! {
                    recv(status_rx) -> msg => match msg {
                        Ok(resp) => resp,
                        _ => Err(Error::BackendNotInitialized),
                    },
                };
                self.chan = Some(Channel {
                    tx: control_tx,
                    rx: status_rx,
                });
                res.and(Ok(()))
            }
            Err(_) => Err(Error::BackendNotInitialized),
        }
    }

    fn close(&self) -> backend::Result {
        if let Some(ref chan) = self.chan {
            if chan.tx.try_send(Control::Stop).is_err() {
                return Err(Error::Internal("close failed".to_owned()));
            }
            let timeout = after(RECV_TIMEOUT);
            select! {
                recv(chan.rx) -> _ => println!("chromecast shutdown: stopped media"),
                recv(timeout) -> _ => println!("chromecast shutdown: failed to stop media"),
            }
            if chan.tx.try_send(Control::Close).is_err() {
                return Err(Error::Internal("close failed".to_owned()));
            }
            let timeout = after(RECV_TIMEOUT);
            select! {
                recv(chan.rx) -> _ => println!("chromecast shutdown: closed device"),
                recv(timeout) -> _ => println!("chromecast shutdown: failed to close device"),
            }
        }
        Ok(())
    }

    fn play(&self, track: Track) -> backend::Result {
        let (ref chan, addr) = match (self.chan.as_ref(), self.media_server_bind_addr) {
            (Some(chan), Some(addr)) => (chan, addr),
            _ => return Err(Error::BackendNotInitialized),
        };
        let track = CastTrack {
            root: self.game_config.root(),
            server: addr,
            track,
        };
        let media = match track.media() {
            Some(media) => media,
            _ => return Err(Error::CannotLoadMedia(track.track)),
        };

        let media = Media {
            content_id: media,
            // Let the device decide whether to buffer or not.
            stream_type: StreamType::None,
            content_type: tree_magic::from_filepath(track.track.path()),
            metadata: track.metadata().map(Metadata::MusicTrack),
            duration: Some(self.game_config.duration.as_fractional_secs() as f32),
        };
        if chan.tx.try_send(Control::Load(Box::new(media))).is_err() {
            return Err(Error::CannotLoadMedia(track.track));
        }
        let timeout = after(self.game_config.duration);
        loop {
            select! {
                recv(chan.rx) -> msg => match msg {
                    Ok(Err(err)) => return Err(err),
                    Err(err) => return Err(
                        Error::Internal(
                            format!("cast communication error: {:?}", err).to_owned()
                        )
                    ),
                    _ => {},
                },
                recv(timeout) -> _ => return Ok(()),
            }
        }
    }
}

/// An iterator yielding Chromecast `Device`s available for audio playback.
///
/// See [`devices()`](fn.devices.html).
pub struct Devices {
    connect: std::collections::hash_set::IntoIter<CastAddr>,
    game: Config,
}

impl Iterator for Devices {
    type Item = Device;

    fn next(&mut self) -> Option<Self::Item> {
        self.connect.next().map(|connect_config| Device {
            connect_config,
            game_config: self.game.clone(),
            chan: None,
            media_server_bind_addr: None,
        })
    }
}

/// An iterator yielding Chromecast `Device`s available for audio playback.
pub fn devices(game_config: Config) -> Devices {
    let mut devices = HashSet::new();
    if let Ok(discovery) = mdns::discover::all(SERVICE_NAME) {
        for response in discovery.timeout(DISCOVER_TIMEOUT) {
            if let Ok(response) = response {
                let mut addr = None;
                let mut port = None;
                let mut metadata = HashMap::new();

                for record in response.records() {
                    match record.kind {
                        RecordKind::A(v4) => addr = Some(v4.into()),
                        RecordKind::AAAA(v6) => addr = Some(v6.into()),
                        RecordKind::SRV { port: p, .. } => port = Some(p),
                        RecordKind::TXT(ref text) => metadata.extend(parser::dns_txt(text)),
                        _ => (),
                    }
                }
                let name = metadata.get(CHROMECAST_NAME_KEY).map(|s| s.to_string());
                if let (Some(name), Some(addr), Some(port)) = (name, addr, port) {
                    println!("{:?} {:?} {:?}", name, addr, port);
                    devices.insert(CastAddr {
                        name,
                        addr: SocketAddr::new(addr, port),
                    });
                }
            }
        }
    }
    Devices {
        connect: devices.into_iter(),
        game: game_config,
    }
}

use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use futures::sync::mpsc::UnboundedReceiver;
use mdns::RecordKind;
use url::Url;

use backend::{self, Error, PlayerKind};
use playlist::{Config, Track};

mod media_server;
mod parser;
use cast::{self, Chromecast, Image, Media};

/// Google Chromecast multicast service identifier.
const SERVICE_NAME: &str = "_googlecast._tcp.local";
/// Key in DNS TXT record for Chromecast "friendly name".
const CHROMECAST_NAME_KEY: &str = "fn";
/// Timeout for discovering Chromecast devices with mdns.
const DISCOVER_TIMEOUT: Duration = Duration::from_millis(3000);

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

    pub fn metadata(&self) -> Option<Media> {
        let url = self.image().and_then(|url| Url::parse(&url).ok());
        let dimensions = self
            .track
            .cover()
            .and_then(|img| img.dimensions().map(|(w, h, _)| (w, h)));
        let cover = match (url, dimensions) {
            (Some(url), Some(dimensions)) => Some(Image { url, dimensions }),
            _ => None,
        };
        let tags = self.track.tags();
        let url = self.media().and_then(|url| Url::parse(&url).ok());
        match (tags, url) {
            (Some(tags), Some(url)) => Some(Media {
                title: tags.title.to_option(),
                artist: tags.artist.to_option(),
                album: tags.album.to_option(),
                url,
                cover,
                content_type: self.track.content_type(),
            }),
            _ => None,
        }
    }
}

pub struct Device {
    game_config: Config,
    connect_config: CastAddr,
    pub cast: Option<Chromecast>, // TODO: don't expose this
    media_server_bind_addr: Option<SocketAddr>,
}

impl Device {
    pub fn name(&self) -> String {
        self.connect_config.name().to_owned()
    }

    pub fn kind(&self) -> PlayerKind {
        PlayerKind::Chromecast
    }

    pub fn connect(
        &mut self,
        rt: &mut tokio::runtime::Runtime,
    ) -> Result<UnboundedReceiver<cast::Status>, backend::Error> {
        match media_server::spawn(self.game_config.root(), self.connect_config.addr) {
            Ok(addr) => {
                self.media_server_bind_addr = Some(addr);
                let (cast, status) = cast::connect(self.connect_config.addr, rt);
                cast.launch_app();
                self.cast = Some(cast);
                Ok(status)
            }
            Err(_) => Err(Error::BackendNotInitialized),
        }
    }

    pub fn stop(&self, connect: &cast::MediaConnection) -> backend::Result {
        let cast = self.cast.as_ref().ok_or(Error::BackendNotInitialized)?;
        cast.stop(connect);
        Ok(())
    }

    pub fn shutdown(&mut self) -> backend::Result {
        let cast = self.cast.take();
        if let Some(cast) = cast {
            cast.shutdown();
        }
        Ok(())
    }

    pub fn load(&self, connect: &cast::ReceiverConnection, track: Track) -> backend::Result {
        let cast = self.cast.as_ref().ok_or(Error::BackendNotInitialized)?;
        let addr = self
            .media_server_bind_addr
            .ok_or(Error::BackendNotInitialized)?;
        let track = CastTrack {
            root: self.game_config.root(),
            server: addr,
            track,
        };
        let media = track
            .metadata()
            .ok_or_else(|| Error::CannotLoadMedia(track.track))?;
        cast.load(connect, media);
        Ok(())
    }

    pub fn play(&self, connect: &cast::MediaConnection) -> backend::Result {
        let cast = self.cast.as_ref().ok_or(Error::BackendNotInitialized)?;
        cast.play(connect);
        Ok(())
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
            cast: None,
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

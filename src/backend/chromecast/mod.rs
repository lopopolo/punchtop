use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::net::SocketAddr;
use std::time::Duration;

use futures::sync::mpsc::UnboundedReceiver;
use mdns::RecordKind;

use backend::{self, Error};
use playlist::fs::Track;

mod media_server;
mod parser;
use self::media_server::Route;
use cast::{self, Chromecast, Image};

/// Google Chromecast multicast service identifier.
const SERVICE_NAME: &str = "_googlecast._tcp.local";
/// Key in DNS TXT record for Chromecast "friendly name".
const CHROMECAST_NAME_KEY: &str = "fn";
/// Timeout for discovering Chromecast devices with mdns.
const DISCOVER_TIMEOUT: Duration = Duration::from_millis(3000);

/// Configuration for Chromecast endpoints.
#[derive(Debug)]
pub struct CastAddr {
    /// Name of a Chromecast as given by the `fn` field in its DNS TXT record.
    pub name: String,
    /// Address of Chromecast as discovered by mdns.
    addr: SocketAddr,
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

#[derive(Debug)]
pub struct Device {
    router: Route,
    cast: Chromecast,
}

impl Device {
    pub fn connect(
        config: &CastAddr,
        registry: HashMap<String, Track>,
        rt: &mut tokio::runtime::Runtime,
    ) -> Result<(Self, UnboundedReceiver<cast::Status>), backend::Error> {
        let router =
            media_server::spawn(registry, config.addr).map_err(|_| Error::BackendNotInitialized)?;
        let (cast, status) = cast::connect(config.addr, rt);
        cast.launch_app();
        let backend = Self { router, cast };
        Ok((backend, status))
    }

    pub fn stop(&self, connect: &cast::MediaConnection) -> backend::Result {
        self.cast.stop(connect);
        Ok(())
    }

    pub fn shutdown(&mut self) -> backend::Result {
        self.cast.shutdown();
        Ok(())
    }

    pub fn load(&self, connect: &cast::ReceiverConnection, track: &Track) -> backend::Result {
        let media = self.metadata(track).ok_or_else(|| Error::CannotLoadMedia)?;
        self.cast.load(connect, media);
        Ok(())
    }

    pub fn pause(&self, connect: &cast::MediaConnection) -> backend::Result {
        self.cast.pause(connect);
        Ok(())
    }

    pub fn play(&self, connect: &cast::MediaConnection) -> backend::Result {
        self.cast.play(connect);
        Ok(())
    }

    fn metadata(&self, track: &Track) -> Option<cast::Media> {
        let url = self.router.cover(track);
        let cover = track
            .cover()
            .and_then(|img| img.dimensions().map(|(w, h, _)| (w, h)))
            .map(|dimensions| Image { url, dimensions });
        let tags = track.tags();
        let url = self.router.media(track);
        tags.map(|tags| cast::Media {
            title: tags.title.to_option(),
            artist: tags.artist.to_option(),
            album: tags.album.to_option(),
            url,
            cover,
            content_type: track.content_type(),
        })
    }
}

/// An iterator yielding Chromecast `Device`s available for audio playback.
///
/// See [`devices()`](fn.devices.html).
pub struct Devices {
    connect: std::collections::hash_set::IntoIter<CastAddr>,
}

impl Iterator for Devices {
    type Item = CastAddr;

    fn next(&mut self) -> Option<Self::Item> {
        self.connect.next()
    }
}

/// An iterator yielding Chromecast `Device`s available for audio playback.
pub fn devices() -> Devices {
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
    }
}

use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::net::SocketAddr;
use std::time::Duration;

use cast_client::{self, Client, Image, Media, MediaConnection, ReceiverConnection, Status};
use futures::sync::mpsc::UnboundedReceiver;
use futures::Future;
use mdns::RecordKind;

mod media_server;
mod parser;

use crate::chromecast::media_server::Route;
use crate::{Error, Result, Track};

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
    fn eq(&self, other: &Self) -> bool {
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
    cast: Client,
}

impl Device {
    pub fn connect(
        config: &CastAddr,
        registry: HashMap<String, Box<dyn Track + Send + Sync>>,
    ) -> std::result::Result<
        (
            Self,
            UnboundedReceiver<Status>,
            impl Future<Item = (), Error = ()>,
        ),
        Error,
    > {
        let router =
            media_server::spawn(registry, config.addr).map_err(|_| Error::BackendNotInitialized)?;
        let (cast, status, connect) = cast_client::connect(config.addr);
        cast.launch_app();
        let backend = Self { router, cast };
        Ok((backend, status, connect))
    }

    pub fn stop(&self, connect: &MediaConnection) -> Result {
        self.cast.stop(connect);
        Ok(())
    }

    pub fn shutdown(&mut self) -> Result {
        self.cast.shutdown();
        Ok(())
    }

    pub fn load(&self, connect: &ReceiverConnection, track: &impl Track) -> Result {
        let media = self.metadata(track).ok_or_else(|| Error::CannotLoadMedia)?;
        self.cast.load(connect, media);
        Ok(())
    }

    pub fn pause(&self, connect: &MediaConnection) -> Result {
        self.cast.pause(connect);
        Ok(())
    }

    pub fn play(&self, connect: &MediaConnection) -> Result {
        self.cast.play(connect);
        Ok(())
    }

    fn metadata(&self, track: &impl Track) -> Option<Media> {
        let url = self.router.cover(track);
        let cover = track
            .cover()
            .map(|img| (img.width, img.height))
            .map(|dimensions| Image { url, dimensions });
        let tags = track.tags();
        let url = self.router.media(track);
        tags.map(|tags| Media {
            title: tags.title,
            artist: tags.artist,
            album: tags.album,
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

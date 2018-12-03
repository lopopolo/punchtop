use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use crossbeam_channel::{after, unbounded, Receiver, Sender};
use floating_duration::TimeAsFloat;
use mdns::RecordKind;
use rust_cast::channels::media::{Media, Metadata, StreamType};

use self::cast::{CastResult, Control};
use backend::{self, media_server, Error, Player, PlayerKind};
use playlist::{Config, Track};

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
        match media_server::spawn(self.game_config.root()) {
            Ok(addr) => {
                self.media_server_bind_addr = Some(addr);

                let (control_tx, control_rx) = unbounded();
                let (status_tx, status_rx) = unbounded();
                let cast_addr = self.connect_config.addr.to_owned();
                thread::spawn(move || cast::runloop(cast_addr, cast::chan(status_tx, control_rx)));

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
        let url_path = PathBuf::from(track.path())
            .strip_prefix(self.game_config.root())
            .ok()
            .and_then(|suffix| suffix.to_str())
            .map(String::from);
        let url_path = match url_path {
            Some(url_path) => url_path,
            None => return Err(Error::CannotLoadMedia(track)),
        };

        let media = Media {
            content_id: format!("http://{}/media/{}", addr, url_path),
            // Let the device decide whether to buffer or not.
            stream_type: StreamType::None,
            content_type: tree_magic::from_filepath(track.path()),
            metadata: cast::metadata(&track, format!("http://{}/image/{}", addr, url_path))
                .map(Metadata::MusicTrack),
            duration: Some(self.game_config.duration.as_fractional_secs() as f32),
        };
        if chan.tx.try_send(Control::Load(Box::new(media))).is_err() {
            return Err(Error::CannotLoadMedia(track));
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

/// Parser for Chromecast TXT records.
///
/// Each Chromecast TXT record is a `key=value` pair that specifies some
/// metadata about the device. There are [several key-value pairs in the record](https://github.com/azasypkin/rust-cast#dns-txt-record-description).
/// The most relevant ones are:
///
/// - `md` - Model Name
/// - `fn` - Friendly Name
mod parser {
    extern crate nom;

    use nom::alphanumeric;
    use nom::types::CompleteStr;

    use std::collections::HashMap;
    use std::str;

    named!(key_value<CompleteStr, (CompleteStr, CompleteStr)>,
    do_parse!(
        key: alphanumeric >>
        char!('=') >>
        val: take_while!(call!(|_| true)) >>
        (key, val)
    )
    );

    /// Extract key-value pairs out of a TXT record and collect them into
    /// a `HashMap`.
    pub fn dns_txt<T: AsRef<str>>(vec: &[T]) -> HashMap<String, String> {
        let mut collect: HashMap<String, String> = HashMap::new();
        for txt in vec.iter() {
            match key_value(CompleteStr(txt.as_ref())) {
                Ok((_, (key, value))) => {
                    collect.insert(key.as_ref().to_owned(), value.as_ref().to_owned())
                }
                _ => None,
            };
        }
        collect
    }

    #[test]
    fn parse_dns_txt() {
        let parsed = dns_txt(&vec!["fn=Device Name=Bob's", "md=Chromecast"]);
        let name = parsed.get("fn").unwrap();
        let model = parsed.get("md").unwrap();
        assert_eq!("Device Name=Bob's", name);
        assert_eq!("Chromecast", model);
        assert_eq!(None, parsed.get("none"));
    }
}

mod cast {
    use std::net::SocketAddr;

    use crossbeam_channel::{Receiver, Sender};
    use rust_cast::channels::media::{Image, Media, MusicTrackMediaMetadata, StatusEntry};
    use rust_cast::channels::receiver::{Application, CastDeviceApp};
    use rust_cast::CastDevice;

    use backend::Error;
    use playlist::Track;

    pub type CastResult = Result<Status, Error>;

    pub struct Channel {
        tx: Sender<CastResult>,
        rx: Receiver<Control>,
    }

    pub enum Control {
        Close,
        Load(Box<Media>),
        Stop,
    }

    pub enum Status {
        Closed,
        Connected,
        Loaded,
        Stopped,
    }

    pub fn chan(tx: Sender<CastResult>, rx: Receiver<Control>) -> Channel {
        Channel { tx, rx }
    }

    pub fn metadata(track: &Track, cover_url: String) -> Option<MusicTrackMediaMetadata> {
        let mut metadata = None;
        if let Some(tags) = track.tags() {
            metadata = Some(MusicTrackMediaMetadata {
                album_name: tags.album.to_option(),
                title: tags.title.to_option(),
                album_artist: tags.album_artist.to_option(),
                artist: tags.artist.to_option(),
                composer: tags.composer.to_option(),
                track_number: Some(1 as u32), // use game cursor
                disc_number: Some(1),
                release_date: tags.date.to_option().map(|d| d.to_iso_8601()),
                images: vec![Image {
                    url: cover_url,
                    dimensions: track
                        .cover()
                        .and_then(|img| img.dimensions().map(|(w, h, _)| (w, h))),
                }],
            });
        }
        metadata
    }

    pub fn runloop(addr: SocketAddr, chan: Channel) {
        let (device, app) = match connect(addr) {
            Ok(connection) => {
                let _ = chan.tx.try_send(Ok(Status::Connected));
                connection
            }
            Err(err) => {
                let _ = chan.tx.send(Err(err));
                return;
            }
        };
        loop {
            select! {
                recv(chan.rx) -> msg => match msg {
                    Ok(Control::Close) => {
                        let close = device
                            .receiver
                            .stop_app(&app.session_id[..])
                            .map_err(Error::Cast)
                            .map(|_| Status::Closed);
                        let _ = chan.tx.try_send(close);
                    },
                    Ok(Control::Load(media)) => {
                        let load = device
                            .media
                            .load(&app.transport_id[..], &app.session_id[..], &media)
                            .map_err(Error::Cast)
                            .map(|_| Status::Loaded);
                        let _ = chan.tx.try_send(load);
                    },
                    Ok(Control::Stop) => {
                        match status(&device, &app) {
                            Ok(entries) => {
                                let mut succeed = true;
                                for entry in entries {
                                    let stop = device
                                        .media
                                        .stop(&app.transport_id[..], entry.media_session_id)
                                        .map_err(Error::Cast);
                                    if let Err(stop) = stop {
                                        let _ = chan.tx.try_send(Err(stop));
                                        succeed = false;
                                    }
                                }
                                if succeed {
                                    let _ = chan.tx.try_send(Ok(Status::Stopped));
                                }
                            }
                            Err(err) => {
                                let _ = chan.tx.try_send(Err(err));
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    fn connect<'a>(addr: SocketAddr) -> Result<(CastDevice<'a>, Application), Error> {
        let ip = format!("{}", addr.ip());
        match CastDevice::connect_without_host_verification(ip, addr.port()) {
            Err(_) => Err(Error::BackendNotInitialized),
            Ok(device) => {
                let sink = CastDeviceApp::DefaultMediaReceiver;
                let app = device
                    .connection
                    .connect("receiver-0")
                    .and_then(|_| device.receiver.launch_app(&sink))
                    .and_then(|app| {
                        device
                            .connection
                            .connect(&app.transport_id[..])
                            .map(|_| app)
                    });
                app.map_err(Error::Cast).map(|app| (device, app))
            }
        }
    }

    pub fn status(device: &CastDevice, app: &Application) -> Result<Vec<StatusEntry>, Error> {
        device
            .media
            .get_status(&app.transport_id[..], None)
            .map_err(Error::Cast)
            .map(|status| status.entries)
    }
}

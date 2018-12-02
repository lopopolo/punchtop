use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};
use std::time::Duration;

use floating_duration::TimeAsFloat;
use mdns::RecordKind;
use neguse_taglib::{get_front_cover, get_tags};
use rust_cast::channels::heartbeat::HeartbeatResponse;
use rust_cast::channels::media::{Image, Media, Metadata, MusicTrackMediaMetadata, StreamType};
use rust_cast::channels::receiver::{Application, CastDeviceApp};
use rust_cast::{CastDevice, ChannelMessage};

use backend::{media_server, Error, Player, PlayerKind};

const SERVICE_NAME: &str = "_googlecast._tcp.local";
const CHROMECAST_NAME_KEY: &str = "fn";

/// Configuration for Chromecast endpoints.
struct CastAddr {
    /// Name of a Chromecast as given by the `fn` field in its DNS TXT record.
    name: String,
    /// IP Address of a Chromecast as discovered by mdns.
    addr: IpAddr,
    /// Port of a Chromecast as discovered by mdns.
    port: u16,
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

pub struct Device<'a> {
    config: CastAddr,
    connection: Option<(CastDevice<'a>, Application)>,
    root: Option<PathBuf>,
    media_server_bind_addr: Option<SocketAddr>,
}

impl<'a> Drop for Device<'a> {
    fn drop(&mut self) {
        let supress: Result<(), ()> = Ok(());
        self.close().or(supress).expect("Closed Chromecast device");
    }
}

impl<'p> Player for Device<'p> {
    fn name(&self) -> String {
        self.config.name().to_owned()
    }

    fn kind(&self) -> PlayerKind {
        PlayerKind::Chromecast
    }

    fn connect<'a>(&mut self, root: &'a Path) -> Result<(), Error<'a>> {
        self.root = Some(PathBuf::from(root));
        match media_server::spawn(root) {
            Ok(addr) => self.media_server_bind_addr = Some(addr),
            err @ Err(_) => return Err(Error::BackendNotInitialized),
        };

        match CastDevice::connect_without_host_verification(
            format!("{}", self.config.addr),
            self.config.port,
        ) {
            Err(_) => Err(Error::BackendNotInitialized),
            Ok(device) => {
                let sink = CastDeviceApp::DefaultMediaReceiver;
                device
                    .connection
                    .connect("receiver-0")
                    .and_then(|_| device.receiver.launch_app(&sink))
                    .and_then(|app| {
                        device
                            .connection
                            .connect(&app.transport_id[..])
                            .map(|_| app)
                    })
                    .map(|app| {
                        self.connection = Some((device, app));
                    })
                    .map_err(|_| Error::BackendNotInitialized)
            }
        }
    }

    fn close<'a>(&self) -> Result<(), Error<'a>> {
        self.connection
            .as_ref()
            .ok_or(Error::BackendNotInitialized)
            .and_then(|(device, app)| {
                device
                    .receiver
                    .stop_app(&app.session_id[..])
                    .map_err(|_| Error::BackendNotInitialized)
            })
    }

    fn play<'a>(&self, path: &'a Path, duration: Duration) -> Result<(), Error<'a>> {
        let addr = match self.media_server_bind_addr {
            Some(addr) => addr,
            None => return Err(Error::PlaybackFailed),
        };
        let pathbuf = PathBuf::from(path);
        let url_path = self
            .root
            .as_ref()
            .and_then(|root| pathbuf.strip_prefix(&root).ok())
            .and_then(|suffix| suffix.to_str())
            .map(String::from);
        let url_path = match url_path {
            Some(url_path) => url_path,
            None => return Err(Error::PlaybackFailed),
        };

        let mut metadata = None;
        let tags = get_tags(path).ok();
        let cover = get_front_cover(path);
        if let Some(tags) = tags {
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
                    url: format!("http://{}/image/{}", addr, url_path).to_owned(),
                    dimensions: cover
                        .ok()
                        .and_then(|img| img.dimensions().map(|(w, h, _)| (w, h))),
                }],
            });
        }
        let media = Media {
            content_id: format!("http://{}/media/{}", addr, url_path),
            // Let the device decide whether to buffer or not.
            stream_type: StreamType::None,
            content_type: tree_magic::from_filepath(path),
            metadata: metadata.map(Metadata::MusicTrack),
            duration: Some(duration.as_fractional_secs() as f32),
        };
        let device = self
            .connection
            .as_ref()
            .ok_or(Error::BackendNotInitialized)
            .and_then(|(device, app)| {
                device
                    .media
                    .load(&app.transport_id[..], &app.session_id[..], &media)
                    .map_err(|e| Error::CannotLoadMedia(path))
                    .map(|_| (device, app))
            });

        if let Ok((ref device, ref app)) = device {
            'receive: loop {
                let recv = match device.receive() {
                    Ok(ChannelMessage::Heartbeat(HeartbeatResponse::Ping)) => device
                        .heartbeat
                        .pong()
                        .map_err(|_| Error::PlaybackFailed)
                        .map(|_| ()),
                    Ok(ChannelMessage::Connection(_))
                    | Ok(ChannelMessage::Media(_))
                    | Ok(ChannelMessage::Receiver(_))
                    | Ok(ChannelMessage::Raw(_)) => Ok(()),
                    _ => Err(Error::PlaybackFailed),
                };
                if recv.is_err() {
                    return recv;
                }
                match device.media.get_status(&app.transport_id[..], None) {
                    Ok(status) => {
                        for entry in status.entries {
                            if let Some(elapsed) = entry.current_time {
                                if (duration.as_fractional_secs() as f32) < elapsed {
                                    device
                                        .media
                                        .stop(&app.transport_id[..], entry.media_session_id)
                                        .ok()
                                        .unwrap();
                                    break 'receive;
                                }
                            }
                        }
                    }
                    Err(_) => return Err(Error::PlaybackFailed),
                }
            }
        }
        device.map(|_| ())
    }
}

/// An iterator yielding Chromecast `Device`s available for audio playback.
///
/// See [`devices()`](fn.devices.html).
pub struct Devices<'a>(
    std::collections::hash_set::IntoIter<CastAddr>,
    PhantomData<&'a CastAddr>,
);

impl<'a> Iterator for Devices<'a> {
    type Item = Device<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|config| Device {
            config,
            connection: None,
            root: None,
            media_server_bind_addr: None,
        })
    }
}

/// An iterator yielding Chromecast `Device`s available for audio playback.
pub fn devices<'a>() -> Devices<'a> {
    let mut devices = HashSet::new();
    if let Ok(discovery) = mdns::discover::all(SERVICE_NAME) {
        for response in discovery.timeout(Duration::from_millis(1000)) {
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
                    devices.insert(CastAddr { name, addr, port });
                }
            }
        }
    }
    Devices(devices.into_iter(), PhantomData)
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

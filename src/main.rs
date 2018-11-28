#![feature(inner_deref)]

extern crate mdns;
extern crate mp3_duration;
#[macro_use]
extern crate nom;
extern crate rand;
extern crate rodio;
extern crate taglib;
extern crate walkdir;

mod parser;
mod playlist;

use mdns::RecordKind;
use std::collections::HashMap;
use std::net::IpAddr;
use std::ops::Deref;
use std::path::Path;
use std::sync::{RwLock, Arc};
use std::thread;
use std::time::Duration;

const SERVICE_NAME: &'static str = "_googlecast._tcp.local";

struct ChromecastConfig {
    addr: Option<IpAddr>,
    txt: HashMap<String, String>,
}

impl ChromecastConfig {
    pub fn name(&self) -> Option<&str> {
        self.txt.get("fn").map(|n| n.deref())
    }
}

fn spawn_mdns(registry: Arc<RwLock<HashMap<String, ChromecastConfig>>>) {
    thread::spawn(move || {
        for response in mdns::discover::all(SERVICE_NAME).unwrap() {
            let response = response.unwrap();

            let mut config = ChromecastConfig { addr: None, txt: HashMap::new() };
            for record in response.records() {
                match record.kind {
                    RecordKind::A(addr) => config.addr = Some(addr.into()),
                    RecordKind::AAAA(addr) => config.addr = Some(addr.into()),
                    RecordKind::TXT(ref text) => {
                        let refs = text.iter().map(|s| s.deref()).collect();
                        config.txt = parser::dns_txt(refs);
                    },
                    _ => (),
                }
            }
            let name = config.name().map(String::from);
            if let Some(name) = name {
                match registry.write() {
                    Ok(mut map) => map.insert(name, config),
                    _ => None,
                };
            }
        }
    });
}

fn main() {
    let registry = Arc::new(RwLock::new(HashMap::new()));
    spawn_mdns(Arc::clone(&registry));

    let device = rodio::default_output_device().unwrap();
    let sink = rodio::Sink::new(&device);

    let config = playlist::Config::new(Duration::new(5, 0), 10);
    let playlist =
        playlist::Playlist::from_directory(Path::new("/Users/lopopolo/Downloads/test"), config);
    for track in playlist {
        match (track.metadata.artist(), track.metadata.album(), track.metadata.title()) {
            (Some(artist), Some(album), Some(title)) => {
                println!("{}", title);
                println!("{} -- {}", artist, album);
            },
            (Some(artist), None, Some(title)) => {
                println!("{}", title);
                println!("{}", artist);
            },
            (None, None, Some(title)) => {
                println!("{}", title);
            },
            _ => (),
        }
        match registry.read() {
            Ok(map) => println!("{:?}", map.keys()),
            _ => (),
        };
        sink.append(track.stream());
        sink.sleep_until_end();
    }
}

use backend::{Error, Player};
use mdns::RecordKind;
use std::collections::{HashMap, HashSet};
use std::net::IpAddr;
use std::path::Path;
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;

const SERVICE_NAME: &str = "_googlecast._tcp.local";
const CHROMECAST_NAME_KEY: &str = "fn";

lazy_static! {
    static ref DISCOVERY: Discovery = Discovery::new();
}

/// Configuration for Chromecast endpoints.
#[derive(Debug)]
struct Config {
    /// Name of a Chromecast as given by the `fn` field in its DNS TXT record.
    name: Option<String>,
    /// IP Address of a Chromecast as discovered by mdns.
    addr: IpAddr,
}

impl Config {
    pub fn name(&self) -> Option<&str> {
        self.name.deref()
    }
}

#[derive(Debug)]
pub struct Device {
    config: Config,
}

impl Player for Device {
    fn name(&self) -> String {
        self.config.name().unwrap_or_else(|| "Chromecast").to_owned()
    }

    fn play<'a, T: AsRef<Path>>(&self, path: &'a T, duration: Duration) -> Result<(), Error<'a>> {
        Ok(())
    }
}

/// Service discovery for Chromecasts on the network.
///
/// Spawn one global instance of `Discovery` to scan multicast DNS broadcasts
/// and update a global registry of Chromecast config.
pub struct Discovery {
    registry: Arc<RwLock<HashSet<(Option<String>, IpAddr)>>>,
}

impl Discovery {
    /// Spawn a thread to run mdns discovery in a loop.
    fn new() -> Self {
        let registry = Arc::new(RwLock::new(HashSet::new()));
        spawn_mdns(Arc::clone(&registry));
        Discovery { registry }
    }

    pub fn poll() -> Vec<Device> {
        DISCOVERY
            .registry
            .read()
            .map(|registry| {
                registry
                    .iter()
                    .map(|(name, addr)| Device {
                        config: Config {
                            name: name.as_ref().map(String::clone),
                            addr: addr.to_owned()
                        }
                    })
                    .collect()
            })
            .unwrap_or_else(|_| vec![])
    }
}

/// Worker thread that polls mDNS and updates the global Chromecast registry.
fn spawn_mdns(registry: Arc<RwLock<HashSet<(Option<String>, IpAddr)>>>) {
    thread::spawn(move || {
        for response in mdns::discover::all(SERVICE_NAME).unwrap() {
            if let Ok(response) = response {
                let mut device_addr = None;
                let mut txt: HashMap<String, String> = HashMap::new();
                for record in response.records() {
                    match record.kind {
                        RecordKind::A(addr) => device_addr = Some(addr.into()),
                        RecordKind::AAAA(addr) => device_addr = Some(addr.into()),
                        RecordKind::TXT(ref text) => txt.extend(parser::dns_txt(text)),
                        _ => (),
                    }
                }
                let name = txt.get(CHROMECAST_NAME_KEY).map(|s| s.to_string());
                if let Some(addr) = device_addr {
                    if let Ok(mut set) = registry.write() {
                        set.insert((name, addr));
                    }
                }
            }
        }
    });
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

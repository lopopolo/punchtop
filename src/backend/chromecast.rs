use backend;
use backend::Error;
use mdns::RecordKind;
use std::collections::HashMap;
use std::net::IpAddr;
use std::ops::Deref;
use std::path::Path;
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;

const SERVICE_NAME: &str = "_googlecast._tcp.local";
const CHROMECAST_NAME_KEY: &str = "fn";

struct Config {
    pub addr: IpAddr,
    txt: HashMap<String, String>,
}

impl Config {
    pub fn name(&self) -> Option<&str> {
        self.txt.get(CHROMECAST_NAME_KEY).map(|n| n.deref())
    }
}

pub struct BackendDevice {
    config: Config,
}

impl backend::BackendDevice for BackendDevice {
    fn play<'a>(&self, path: &'a Path, duration: Duration) -> Result<(), Error<'a>> {
        Ok(())
    }
}

pub struct Discovery {
    registry: Arc<RwLock<HashMap<String, Config>>>,
}

impl Discovery {
    pub fn new() -> Self {
        let registry = Arc::new(RwLock::new(HashMap::new()));
        spawn_mdns(Arc::clone(&registry));
        Discovery { registry }
    }

    pub fn poll(&self) -> Vec<String> {
        self.registry.read()
            .map(|map| map.keys().map(|name| name.to_owned()).collect())
            .unwrap_or_else(|_| vec![])
    }

    pub fn backend(&self, name: &str) -> Option<impl backend::BackendDevice> {
        let d: Option<BackendDevice> = None;
        d
    }
}

fn spawn_mdns(registry: Arc<RwLock<HashMap<String, Config>>>) {
    thread::spawn(move || {
        for response in mdns::discover::all(SERVICE_NAME).unwrap() {
            let response = response.unwrap();

            let mut device_addr = None;
            let mut txt: HashMap<String, String> = HashMap::new();
            for record in response.records() {
                match record.kind {
                    RecordKind::A(addr) => device_addr = Some(addr.into()),
                    RecordKind::AAAA(addr) => device_addr = Some(addr.into()),
                    RecordKind::TXT(ref text) => {
                        let refs: Vec<&str> = text.iter().map(|s| s.deref()).collect();
                        txt.extend(parser::dns_txt(&refs));
                    }
                    _ => (),
                }
            }
            let name = txt.get(CHROMECAST_NAME_KEY).map(|s| s.to_string());
            if let (Some(addr), Some(name)) = (device_addr, name) {
                if let Ok(mut map) = registry.write() {
                    map.insert(name, Config { addr, txt });
                }
            }
        }
    });
}

mod parser {
    extern crate nom;

    use nom::alphanumeric;
    use nom::types::CompleteByteSlice;

    use std::collections::HashMap;
    use std::str;

    fn complete_byte_slice_to_str(s: CompleteByteSlice) -> Result<&str, str::Utf8Error> {
        str::from_utf8(s.0)
    }

    named!(key_value<CompleteByteSlice, (&str, &str)>,
      do_parse!(
          key: map_res!(alphanumeric, complete_byte_slice_to_str)
      >>       char!('=')
      >>  val: map_res!(
               take_while!(call!(|_| true)),
               complete_byte_slice_to_str
             )
      >>     (key, val)
      )
    );

    /// TXT records are given as a Vec of key=value pairs
    pub fn dns_txt(vec: &[&str]) -> HashMap<String, String> {
        let mut collect: HashMap<String, String> = HashMap::new();
        for txt in vec.iter() {
            match key_value(CompleteByteSlice(txt.as_bytes())) {
                Ok((_, (key, value))) => collect.insert(key.to_owned(), value.to_owned()),
                _ => None,
            };
        }
        collect
    }
}

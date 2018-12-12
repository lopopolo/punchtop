///! An embedded media server for making tracks and cover art available to a
///! Chromecast.
use std::collections::HashMap;
use std::io::{Cursor, Read};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::RwLock;
use std::thread;

use rocket::config::{Config, Environment};
use rocket::response::Stream;
use rocket::State;

use backend::chromecast::Media;
use playlist::Track;

/// Media server error wrapper.
#[derive(Debug)]
pub enum Error {
    /// No interfaces available to bind to.
    NoBindInterfaces,
    /// No ports available to bind to on selected interface.
    NoBindPort,
}

#[derive(Clone, Debug)]
pub struct Route(pub SocketAddr);

impl Route {
    pub fn media(&self, media: &Media) -> String {
        format!("http://{}/{}", self.0, uri!(media: media.track.id()))
    }

    pub fn cover(&self, media: &Media) -> String {
        format!("http://{}/{}", self.0, uri!(cover: media.track.id()))
    }
}

struct TrackRegistry(RwLock<HashMap<String, Track>>);

#[get("/media/<id>")]
fn media(id: String, state: State<TrackRegistry>) -> Option<Stream<Cursor<Vec<u8>>>> {
    state.0.read()
        .ok()
        .and_then(|registry| registry.get(&id).and_then(|track| track.stream()))
        .and_then(|mut stream| {
            let mut buf = Vec::new();
            match stream.read_to_end(&mut buf) {
                Ok(_) => Some(buf),
                Err(_) => None,
            }
        }) // TODO: set Content-Type header
        .map(Cursor::new)
        .map(Stream::from)
}

#[get("/cover/<id>")]
fn cover(id: String, state: State<TrackRegistry>) -> Option<Stream<Cursor<Vec<u8>>>> {
    state.0.read()
        .ok()
        .and_then(|registry| registry.get(&id).and_then(|track| track.cover()))
        .map(|img| img.unwrap()) // TODO: set Content-Type header
        .map(Cursor::new)
        .map(Stream::from)
}

/// Spawn a thread that runs a media server for the given track registry.
pub fn spawn(registry: HashMap<String, Track>, cast: SocketAddr) -> Result<SocketAddr, Error> {
    let addr = default_interface_addr(cast).and_then(get_available_port)?;
    debug!("bind to {:?}", addr);
    // TODO: call `set_secret_key` with a base64-encoded 256-bit random value
    // to address a warning from rocket.
    let config = Config::build(Environment::Production)
        .address(format!("{}", addr.ip()))
        .port(addr.port())
        .unwrap();
    thread::spawn(|| {
        rocket::custom(config)
            .manage(TrackRegistry(RwLock::new(registry)))
            .mount("/", routes![media, cover]).launch();
    });
    Ok(addr)
}

/// Find the socket address of the default network interface used to
/// connect to the chromecast at `addr`.
///
/// Used as bind address for the media server.
fn default_interface_addr(addr: SocketAddr) -> Result<SocketAddr, Error> {
    TcpStream::connect(addr)
        .and_then(|conn| conn.local_addr())
        .map_err(|_| Error::NoBindInterfaces)
}

fn port_is_available(addr: SocketAddr) -> bool {
    TcpListener::bind(addr).is_ok()
}

fn get_available_port(addr: SocketAddr) -> Result<SocketAddr, Error> {
    (1025..65535)
        .map(|port| {
            let mut candidate = addr;
            candidate.set_port(port);
            candidate
        })
        .find(|addr| port_is_available(*addr))
        .ok_or(Error::NoBindPort)
}

use std::fs;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::thread;

use rouille::{self, Request, Response};

use playlist::Track;

/// Media server error wrapper.
#[derive(Debug)]
pub enum Error {
    /// No interfaces available to bind to.
    NoBindInterfaces,
    /// No ports available to bind to on selected interface.
    NoBindPort,
    /// Internal rouille error.
    ServerFailedToStart(Box<std::error::Error + Send + Sync>),
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

/// Spawn a thread that runs a static asset server rooted at `root`.
pub fn spawn(root: &Path, cast: SocketAddr) -> Result<SocketAddr, Error> {
    let document_root = PathBuf::from(root);
    default_interface_addr(cast)
        .and_then(get_available_port)
        .and_then(|addr| {
            println!("mount media directory root={:?}", document_root);
            rouille::Server::new(addr, move |request| {
                println!("request={:?}", request);
                if let Some(request) = request.remove_prefix("/media") {
                    rouille::match_assets(&request, &document_root)
                } else if let Some(request) = request.remove_prefix("/image") {
                    image_assets(&request, &document_root)
                } else {
                    Response::empty_404()
                }
            })
            .map_err(Error::ServerFailedToStart)
        })
        .map(|server| {
            let addr = server.server_addr();
            println!("spawn http thread bind={:?}", addr);
            thread::spawn(move || server.run());
            addr
        })
}

/// Controller for serving album art.
fn image_assets(request: &Request, root: &Path) -> Response {
    let potential_file = {
        let mut path = root.to_path_buf();
        for component in request.url().split('/') {
            path.push(component);
        }
        path
    };
    let potential_file = match potential_file.canonicalize() {
        Ok(f) => f,
        Err(_) => return Response::empty_404(),
    };
    // Prevent directory traversal attacks
    if !potential_file.starts_with(&root) {
        return Response::empty_404();
    }
    match fs::metadata(&potential_file) {
        Ok(ref m) if m.is_file() => (),
        _ => return Response::empty_404(),
    };
    match fs::File::open(&potential_file) {
        Ok(_) => (),
        Err(_) => return Response::empty_404(),
    };
    Track::new(potential_file)
        .cover()
        .map(|img| Response::from_data(img.mime(), img.unwrap()))
        .unwrap_or_else(Response::empty_404)
}

/// Find the socket address of the default network interface used to
/// connect to the chromecast at `addr`.
///
/// Used as bind address for `rouille`.
fn default_interface_addr(addr: SocketAddr) -> Result<SocketAddr, Error> {
    TcpStream::connect(addr)
        .and_then(|conn| conn.local_addr())
        .map_err(|_| Error::NoBindInterfaces)
}

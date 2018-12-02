use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::thread;

use interfaces::{Interface, Kind};
use neguse_taglib::get_front_cover;
use rouille::{self, Request, Response};

/// Media server error wrapper.
#[derive(Debug)]
pub enum Error {
    /// No interfaces available to bind to.
    NoBindInterfaces,
    /// Internal rouille error.
    ServerFailedToStart(Box<std::error::Error + Send + Sync>),
}

/// Spawn a thread that runs a static asset server rooted at `root`.
pub fn spawn(root: &Path) -> Result<SocketAddr, Error> {
    let document_root = PathBuf::from(root);
    default_interface_addr()
        .and_then(|mut addr| {
            addr.set_port(3000);
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
    let img = match get_front_cover(potential_file) {
        Ok(img) => {
            if img.is_none() {
                return Response::empty_404();
            }
            img
        }
        _ => return Response::empty_404(),
    };
    Response::from_data(img.mime(), img.unwrap())
}

/// [WIP] Find the socket address of the default network interface.
///
/// Used as bind address for `rouille`.
fn default_interface_addr() -> Result<SocketAddr, Error> {
    Interface::get_all()
        .map_err(|_| Error::NoBindInterfaces)
        .map(|interfaces| {
            interfaces.into_iter().filter_map(|i| {
                if i.is_up() && !i.is_loopback() && i.name == "en0" {
                    i.addresses
                        .iter()
                        .find(|a| a.kind == Kind::Ipv4)
                        .and_then(|a| a.addr)
                } else {
                    None
                }
            })
        })
        .and_then(|addrs| addrs.take(1).next().ok_or(Error::NoBindInterfaces))
}

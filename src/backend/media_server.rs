use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::thread;

use interfaces::{Interface, Kind};
use rouille;

const LOG_TARGET: &str = "Chromecast-Asset-Server";

#[derive(Debug)]
pub enum Error {
    NoBindInterfaces,
    ServerFailedToStart(Box<std::error::Error + Send + Sync>),
}

pub fn spawn(root: &Path) -> Result<SocketAddr, Error> {
    let document_root = PathBuf::from(root);
    default_interface_addr()
        .and_then(|mut addr| {
            addr.set_port(3000);
            println!("mount media directory root={:?}", document_root);
            rouille::Server::new(addr, move |request| {
                println!("request={:?}", request);
                rouille::match_assets(request, &document_root)
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

fn default_interface_addr() -> Result<SocketAddr, Error> {
    Interface::get_all()
        .map_err(|_| Error::NoBindInterfaces)
        .map(|interfaces| {
            interfaces.into_iter().filter_map(|i| {
                if i.is_up() && !i.is_loopback() {
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

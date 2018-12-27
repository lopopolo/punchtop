#![feature(inner_deref, try_from)]

#[macro_use]
extern crate log;

use std::io;
use std::net::SocketAddr;

use futures::prelude::*;
use futures::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use futures::{future, Future, Stream};
use futures_locks::RwLock;
use stream_util::{self, Drainable, Trigger};
use tokio_codec::Framed;
use tokio_tcp::TcpStream;
use tokio_tls::{TlsConnector, TlsStream};

mod channel;
mod codec;
#[allow(clippy::all, clippy::pedantic)]
mod proto;
mod provider;
mod session;
mod task;

pub use self::provider::*;

#[derive(Debug)]
pub struct Client {
    command: UnboundedSender<Command>,
    shutdown: Option<Trigger>,
    status: UnboundedSender<Status>,
    connect: RwLock<ConnectState>,
}

impl Client {
    pub fn launch_app(&self) {
        let launch = Command::Launch {
            app_id: channel::DEFAULT_MEDIA_RECEIVER_APP_ID.to_owned(),
        };
        let _ = self
            .command
            .unbounded_send(Command::Connect(ReceiverConnection {
                session: channel::DEFAULT_DESTINATION_ID.to_owned(),
                transport: channel::DEFAULT_DESTINATION_ID.to_owned(),
            }))
            .and_then(|_| self.command.unbounded_send(launch));
    }

    pub fn load(&self, connect: &ReceiverConnection, media: Media) {
        let command = self.command.clone();
        let connect = connect.clone();
        let task = session::invalidate(&self.connect);
        let task = task.and_then(move |_| {
            command
                .unbounded_send(Command::Load {
                    connect,
                    media: Box::new(media),
                })
                .map_err(|_| ())
        });
        tokio_executor::spawn(task);
    }

    pub fn pause(&self, connect: &MediaConnection) {
        let _ = self.command.unbounded_send(Command::Pause(connect.clone()));
    }

    pub fn play(&self, connect: &MediaConnection) {
        let _ = self.command.unbounded_send(Command::Play(connect.clone()));
    }

    pub fn stop(&self, connect: &MediaConnection) {
        let _ = self.command.unbounded_send(Command::Stop(connect.clone()));
    }

    pub fn shutdown(&mut self) {
        let trigger = self.shutdown.take();
        if let Some(trigger) = trigger {
            trigger.terminate();
        }
        if !self.command.is_closed() {
            let _ = self.command.close();
        }
    }
}

/// Asynchronously establish a TLS connection.
fn tls_connect(addr: SocketAddr) -> impl Future<Item = TlsStream<TcpStream>, Error = io::Error> {
    let connector = native_tls::TlsConnector::builder()
        .danger_accept_invalid_hostnames(true)
        .danger_accept_invalid_certs(true)
        .build()
        .map(TlsConnector::from)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e));
    let connector = match connector {
        Ok(connector) => connector,
        Err(err) => return future::Either::A(future::err(err)),
    };
    let connect = TcpStream::connect(&addr)
        .map_err(|err| io::Error::new(io::ErrorKind::Other, err))
        .and_then(move |socket| {
            info!("Establishing TLS connection to {:?}", addr);
            connector
                .connect(&addr.ip().to_string(), socket)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
        });
    future::Either::B(connect)
}

pub fn connect(
    addr: SocketAddr,
) -> (
    Client,
    UnboundedReceiver<Status>,
    impl Future<Item = (), Error = ()>,
) {
    let (command_tx, command_rx) = mpsc::unbounded();
    let (status_tx, status_rx) = mpsc::unbounded();

    let (trigger, valve) = stream_util::valve();

    let connect = RwLock::new(ConnectState::default());
    let cast = Client {
        command: command_tx.clone(),
        shutdown: Some(trigger),
        status: status_tx.clone(),
        connect: connect.clone(),
    };
    let init = tls_connect(addr).map(move |socket| {
        info!("TLS connection established");
        let (sink, source) = Framed::new(socket, codec::CastMessage::default()).split();
        tokio_executor::spawn(task::respond(
            source,
            connect.clone(),
            command_tx.clone(),
            status_tx.clone(),
        ));
        tokio_executor::spawn(task::send(sink, command_rx.drain(valve.clone())));
        tokio_executor::spawn(task::keepalive(valve.clone(), command_tx.clone()));
        tokio_executor::spawn(task::poll_status(
            valve.clone(),
            connect.clone(),
            command_tx.clone(),
        ));
    });
    let init = init.map_err(|err| warn!("error during cast client init: {:?}", err));
    (cast, status_rx, init)
}

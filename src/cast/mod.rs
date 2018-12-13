use std::io;
use std::net::SocketAddr;

use futures::prelude::*;
use futures::sync::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
use futures::sync::oneshot;
use futures::{future, Future, Stream};
use futures_locks::Mutex;
use tokio::codec::Framed;
use tokio::net::TcpStream;
use tokio_tls::{TlsConnector, TlsStream};

mod codec;
mod message;
mod payload;
mod proto;
mod provider;
mod worker;

use self::codec::CastMessageCodec;
pub use self::payload::*;
pub use self::provider::*;

pub const DEFAULT_MEDIA_RECEIVER_APP_ID: &str = "CC1AD845";

#[derive(Debug)]
pub enum ChannelMessage {
    Connection(Box<connection::Payload>),
    Heartbeat(Box<heartbeat::Payload>),
    Media(Box<media::Payload>),
    Receiver(Box<receiver::Payload>),
}

#[derive(Debug)]
pub struct Chromecast {
    command: UnboundedSender<Command>,
    shutdown: (
        oneshot::Sender<()>,
        oneshot::Sender<()>,
        oneshot::Sender<()>,
    ),
    status: UnboundedSender<Status>,
    connect: Mutex<ConnectState>,
}

impl Chromecast {
    pub fn launch_app(&self) {
        let launch = Command::Launch {
            app_id: DEFAULT_MEDIA_RECEIVER_APP_ID.to_owned(),
        };
        let _ = self
            .command
            .unbounded_send(Command::Connect(ReceiverConnection {
                session: message::DEFAULT_DESTINATION_ID.to_owned(),
                transport: message::DEFAULT_DESTINATION_ID.to_owned(),
            }))
            .and_then(|_| self.command.unbounded_send(launch));
    }

    pub fn load(&self, connect: &ReceiverConnection, media: Media) {
        let command = self.command.clone();
        let connect = connect.clone();
        let task = worker::status::invalidate_media_connection(self.connect.clone());
        let task = task.map(move |_| {
            let _ = command.unbounded_send(Command::Load {
                connect,
                media: Box::new(media),
            });
        });
        tokio::spawn(task);
    }

    pub fn play(&self, connect: &MediaConnection) {
        let _ = self.command.unbounded_send(Command::Play(connect.clone()));
    }

    pub fn stop(&self, connect: &MediaConnection) {
        let _ = self.command.unbounded_send(Command::Stop(connect.clone()));
    }

    pub fn shutdown(mut self) {
        let _ = self.shutdown.0.send(());
        let _ = self.shutdown.1.send(());
        let _ = self.shutdown.2.send(());
        let _ = self.command.close();
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
    rt: &mut tokio::runtime::Runtime,
) -> (Chromecast, UnboundedReceiver<Status>) {
    let (command_tx, command_rx) = unbounded();
    let (status_tx, status_rx) = unbounded();

    let shutdown_writer = oneshot::channel();
    let shutdown_heartbeat = oneshot::channel();
    let shutdown_status = oneshot::channel();

    let connect = Mutex::new(ConnectState::default());
    let cast = Chromecast {
        command: command_tx.clone(),
        shutdown: (shutdown_writer.0, shutdown_heartbeat.0, shutdown_status.0),
        status: status_tx.clone(),
        connect: connect.clone(),
    };
    let shutdown_rx = (shutdown_writer.1, shutdown_heartbeat.1, shutdown_status.1);
    let init = tls_connect(addr).map(move |socket| {
        info!("TLS connection established");
        let (sink, source) = Framed::new(socket, CastMessageCodec::default()).split();
        let read = worker::read::task(
            source,
            connect.clone(),
            status_tx.clone(),
            command_tx.clone(),
        );
        tokio::spawn(read);
        let writer = writer(sink, command_rx)
            .select(shutdown_rx.0.map_err(|_| ()))
            .map_err(|_| ())
            .map(|_| ());
        tokio::spawn(writer);

        let heartbeat = worker::heartbeat::task(command_tx.clone())
            .select(shutdown_rx.1.map_err(|_| ()))
            .map_err(|_| ())
            .map(|_| ());
        tokio::spawn(heartbeat);

        let status = worker::status::task(connect.clone(), command_tx.clone())
            .select(shutdown_rx.2.map_err(|_| ()))
            .map_err(|_| ())
            .map(|_| ());
        tokio::spawn(status);
    });
    rt.spawn(init.map_err(|_| ()));
    (cast, status_rx)
}

fn writer(
    sink: impl Sink<SinkItem = Command, SinkError = io::Error>,
    command: UnboundedReceiver<Command>,
) -> impl Future<Item = (), Error = ()> {
    command
        .forward(sink.sink_map_err(|_| ()))
        .map(|_| ())
        .map_err(|err| warn!("Error on recv: {:?}", err))
}

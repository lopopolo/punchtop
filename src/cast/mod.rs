use std::io;
use std::net::SocketAddr;
use std::time::Duration;

use futures::prelude::*;
use futures::sync::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
use futures::{future, Future};
use tokio::codec::Framed;
use tokio::net::TcpStream;
use tokio::timer::Interval;
use tokio_tls::{TlsConnector, TlsStream};

mod codec;
mod message;
mod payload;
mod proto;
mod provider;

use self::codec::CastMessageCodec;
use self::payload::*;
pub use self::provider::*;

const DEFAULT_MEDIA_RECEIVER_APP_ID: &str = "CC1AD845";

#[derive(Debug)]
pub enum ChannelMessage {
    Connection(connection::Payload),
    Heartbeat(heartbeat::Payload),
    Media(media::Payload),
    Receiver(receiver::Payload),
}

#[derive(Debug)]
pub struct Chromecast {
    pub session: Option<String>,
    pub media_session: Option<i32>,
    command: UnboundedSender<Command>,
    status: UnboundedSender<Status>,
}

impl Chromecast {
    pub fn launch_app(&self) {
        let launch = Command::Launch(DEFAULT_MEDIA_RECEIVER_APP_ID.to_owned());
        let _ = self
            .command
            .unbounded_send(Command::Connect)
            .and_then(|_| self.command.unbounded_send(launch));
    }

    pub fn load(&self, media: Media) {
        if let Some(ref session_id) = self.session {
            let _ = self
                .command
                .unbounded_send(Command::Load(session_id.to_owned(), media));
        }
    }

    pub fn play(&self) {
        debug!("in play: {:?}", self.media_session);
        if let Some(media_session_id) = self.media_session {
            let _ = self.command.unbounded_send(Command::Play(media_session_id));
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
            info!("Establishing TLS connection at {:?}", addr);
            let domain = format!("{}", addr.ip());
            connector
                .connect(&domain, socket)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
        });
    future::Either::B(connect)
}

pub fn connect(addr: SocketAddr, rt: &mut tokio::runtime::Runtime) -> (Chromecast, UnboundedReceiver<Status>) {
    let (command_tx, command_rx) = unbounded();
    let (status_tx, status_rx) = unbounded();

    let cast = Chromecast {
        session: None,
        media_session: None,
        command: command_tx.clone(),
        status: status_tx.clone(),
    };
    let init = tls_connect(addr).map(move |socket| {
        info!("TLS connection established");
        let (sink, source) = Framed::new(socket, CastMessageCodec::new()).split();
        tokio::spawn(reader(source, status_tx.clone(), command_tx.clone()));
        tokio::spawn(writer(sink, command_rx));
        tokio::spawn(heartbeat(command_tx.clone()));
    });
    rt.spawn(init.map_err(|_| ()));
    (cast, status_rx)
}

fn reader(
    source: impl Stream<Item = ChannelMessage, Error = io::Error>,
    status: UnboundedSender<Status>,
    command: UnboundedSender<Command>,
) -> impl Future<Item = (), Error = ()> {
    source
        .for_each(move |message| Ok(read(message, status.clone(), command.clone())))
        .map(|_| ())
        .map_err(|err| warn!("Error on send: {:?}", err))
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

fn heartbeat(heartbeat: UnboundedSender<Command>) -> impl Future<Item = (), Error = ()> {
    Interval::new_interval(Duration::new(5, 0))
        .map(|_| Command::Heartbeat)
        .map_err(|_| ())
        .forward(heartbeat.sink_map_err(|_| ()))
        .map(|_| ())
        .map_err(|err| warn!("Error on heartbeat: {:?}", err))
}

fn read(message: ChannelMessage, tx: UnboundedSender<Status>, command: UnboundedSender<Command>) {
    debug!("Message on receiver channel");
    match message {
        ChannelMessage::Heartbeat(_) => {
            debug!("Got heartbeat");
            let _ = command.unbounded_send(Command::ReceiverStatus);
            let _ = command.unbounded_send(Command::MediaStatus);
        }
        ChannelMessage::Receiver(message) => match message {
            receiver::Payload::ReceiverStatus { status, .. } => {
                debug!("Got receiver status: {:?}", status);
                let session_id = status
                    .applications
                    .iter()
                    .find(|app| app.app_id == DEFAULT_MEDIA_RECEIVER_APP_ID)
                    .map(|app| app.session_id.to_owned());
                if let Some(session_id) = session_id {
                    let _ = tx.unbounded_send(Status::Connected(session_id));
                }
            }
            payload => warn!("Got unknown payload on receiver channel: {:?}", payload),
        },
        ChannelMessage::Media(message) => match message {
            media::Payload::MediaStatus { status, .. } => {
                debug!("Got media status");
                let media_session_id = status.first().map(|status| status.media_session_id);
                if let Some(media_session_id) = media_session_id {
                    let _ = tx.unbounded_send(Status::MediaConnected(media_session_id));
                }
            }
            payload => warn!("Got unknown payload on media channel: {:?}", payload),
        },
        payload => warn!("Got unknown payload: {:?}", payload),
    }
}

use std::io;
use std::net::SocketAddr;
use std::time::Duration;

use futures::prelude::*;
use futures::sync::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
use futures::sync::oneshot;
use futures::{future, Future, Stream};
use futures_locks::Mutex;
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
pub use self::payload::*;
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
    command: UnboundedSender<Command>,
    shutdown: (oneshot::Sender<()>, oneshot::Sender<()>, oneshot::Sender<()>),
    status: UnboundedSender<Status>,
}

impl Chromecast {
    pub fn launch_app(&self) {
        let launch = Command::Launch {
            app_id: DEFAULT_MEDIA_RECEIVER_APP_ID.to_owned()
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
        let _ = self
            .command
            .unbounded_send(Command::Load {
                connect: connect.clone(),
                media
            });
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

    let shutdown_writer = oneshot::channel();
    let shutdown_heartbeat = oneshot::channel();
    let shutdown_status = oneshot::channel();
    let cast = Chromecast {
        command: command_tx.clone(),
        shutdown: (shutdown_writer.0, shutdown_heartbeat.0, shutdown_status.0),
        status: status_tx.clone(),
    };
    let shutdown_rx = (shutdown_writer.1, shutdown_heartbeat.1, shutdown_status.1);
    let connect_state = Mutex::new(ConnectState::default());
    let init = tls_connect(addr).map(move |socket| {
        info!("TLS connection established");
        let (sink, source) = Framed::new(socket, CastMessageCodec::new()).split();
        tokio::spawn(reader(source, connect_state.clone(), status_tx.clone(), command_tx.clone()));
        let writer = writer(sink, command_rx)
            .select(shutdown_rx.0.map_err(|_| ()))
            .map_err(|_| ())
            .map(|_| ());
        tokio::spawn(writer);

        let heartbeat = heartbeat(command_tx.clone())
            .select(shutdown_rx.1.map_err(|_| ()))
            .map_err(|_| ())
            .map(|_| ());
        tokio::spawn(heartbeat);

        let status = status(connect_state.clone(), command_tx.clone())
            .select(shutdown_rx.2.map_err(|_| ()))
            .map_err(|_| ())
            .map(|_| ());
        tokio::spawn(status);
    });
    rt.spawn(init.map_err(|_| ()));
    (cast, status_rx)
}

fn reader(
    source: impl Stream<Item = ChannelMessage, Error = io::Error>,
    connect_state: Mutex<ConnectState>,
    status: UnboundedSender<Status>,
    command: UnboundedSender<Command>,
) -> impl Future<Item = (), Error = ()> {
    source
        .for_each(move |message| {
            Ok(read(message, connect_state.clone(), status.clone(), command.clone()))
        })
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

fn status(state: Mutex<ConnectState>, status: UnboundedSender<Command>) -> impl Future<Item = (), Error = ()> {
    Interval::new_interval(Duration::from_millis(50))
        .map_err(|_| ())
        .and_then(move |_| state.lock())
        .map_err(|_| ())
        .for_each(move |state| {
            let _ = status.unbounded_send(Command::ReceiverStatus);
            if let Some(connect) = state.media_connection() {
                let _ = status.unbounded_send(Command::MediaStatus(connect.clone()));
            }
            Ok(())
        })
        .map_err(|err| warn!("Error on status: {:?}", err))
}

fn read(
    message: ChannelMessage,
    connect_state: Mutex<ConnectState>,
    tx: UnboundedSender<Status>,
    command: UnboundedSender<Command>
) {
    match message {
        ChannelMessage::Heartbeat(_) => trace!("Got heartbeat"),
        ChannelMessage::Receiver(message) => match message {
            receiver::Payload::ReceiverStatus { status, .. } => {
                let app = status
                    .applications
                    .iter()
                    .find(|app| app.app_id == DEFAULT_MEDIA_RECEIVER_APP_ID);
                let session = app.map(|app| app.session_id.to_owned());
                let transport = app.map(|app| app.transport_id.to_owned());
                let connect = connect_state.lock()
                    .map(move |mut state| {
                        trace!("Acquired connect state lock in receiver status");
                        state.set_session(session.deref());
                        let did_connect = session.is_some() &&
                            transport.is_some() &&
                            state.set_transport(transport.deref());
                        if let (Some(ref connect), true) = (state.media_connection(), did_connect) {
                            debug!("Connecting to transport {}", connect.receiver.transport);
                            let _ = tx.unbounded_send(Status::Connected(connect.receiver.clone()));
                            // we've connected to the default receiver. Now connect to
                            // the transport backing the launched app session.
                            let _ = command.unbounded_send(Command::Connect(connect.receiver.clone()));
                            let _ = command.unbounded_send(Command::MediaStatus(connect.clone()));
                        }
                        ()
                    });
                tokio::spawn(connect);
            }
            payload => warn!("Got unknown payload on receiver channel: {:?}", payload),
        },
        ChannelMessage::Media(message) => match message {
            media::Payload::MediaStatus { status, .. } => {
                let connect = connect_state.lock()
                    .map(move |mut state| {
                        trace!("Acquired connect state lock in media status: {:?}", state.media_connection());
                        let status = status.into_iter().next();
                        let media_session = status
                            .as_ref()
                            .map(|status| status.media_session_id);
                        let did_connect = state.set_media_session(media_session.clone());
                        if let Some(connection) = state.media_connection() {
                            if did_connect && connection.session.is_some() {
                                info!("Media session established with session id={:?}", media_session);
                                let _ = tx.unbounded_send(Status::MediaConnected(connection));
                            }
                            if let Some(status) = status {
                                let _ = tx.unbounded_send(Status::MediaStatus(status));
                            }
                        }
                        ()
                    });
                tokio::spawn(connect);
            }
            payload => warn!("Got unknown payload on media channel: {:?}", payload),
        },
        payload => warn!("Got unknown payload: {:?}", payload),
    }
}

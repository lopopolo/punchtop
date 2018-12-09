use std::io;
use std::net::SocketAddr;
use std::time::Duration;

use futures::prelude::*;
use futures::sync::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
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
        let launch = Command::Launch {
            app_id: DEFAULT_MEDIA_RECEIVER_APP_ID.to_owned()
        };
        let _ = self
            .command
            .unbounded_send(Command::Connect {
                destination: message::DEFAULT_DESTINATION_ID.to_owned(),
            })
            .and_then(|_| self.command.unbounded_send(launch));
    }

    pub fn load(&self, transport: String, media: Media) {
        if let Some(ref session) = self.session {
            let _ = self
                .command
                .unbounded_send(Command::Load {
                    session: session.to_owned(),
                    transport,
                    media
                });
        }
    }

    pub fn play(&self, transport: String) {
        if let Some(media_session) = self.media_session {
            let _ = self.command.unbounded_send(Command::Play { media_session, transport });
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
    let connect_state = Mutex::new(ConnectState::default());
    let init = tls_connect(addr).map(move |socket| {
        info!("TLS connection established");
        let (sink, source) = Framed::new(socket, CastMessageCodec::new()).split();
        tokio::spawn(reader(source, connect_state.clone(), status_tx.clone(), command_tx.clone()));
        tokio::spawn(writer(sink, command_rx));
        tokio::spawn(heartbeat(command_tx.clone()));
        tokio::spawn(status(connect_state.clone(), command_tx.clone()));
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
                let _ = status.unbounded_send(Command::MediaStatus {
                    transport: connect.transport.to_owned(),
                });
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
        ChannelMessage::Heartbeat(_) => {
            debug!("Got heartbeat");
            let _ = command.unbounded_send(Command::ReceiverStatus);
        }
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
                        warn!("acquired connect state lock receiver status");
                        let was_connected = state.is_connected();
                        if let (Some(session), None) = (session.as_ref(), state.session.as_ref()) {
                            state.session = Some(session.to_owned());
                        }
                        if let (Some(transport), None) = (transport.as_ref(), state.transport.as_ref()) {
                            state.transport = Some(transport.to_owned());
                        }
                        if let (Some(ref transport), false) = (transport, was_connected) {
                            warn!("connecting to transport {}", transport);
                            // we've connected to the default receiver. Now connect to
                            // the transport backing the launched app session.
                            let _ = command.unbounded_send(Command::Connect {
                                destination: transport.to_owned(),
                            });
                            let _ = command.unbounded_send(Command::MediaStatus {
                                transport: transport.to_owned(),
                            });
                        }
                        ()
                    });
                tokio::spawn(connect);
            }
            payload => warn!("Got unknown payload on receiver channel: {:?}", payload),
        },
        ChannelMessage::Media(message) => match message {
            media::Payload::MediaStatus { status, .. } => {
                debug!("Got media status: {:?}", status);
                let media_session = status.first().map(|status| status.media_session_id);
                let connect = connect_state.lock()
                    .map(move |mut state| {
                        warn!("acquired connect state lock media status: {:?}", state.media_connection());
                        let was_connected = state.media_session.is_some();
                        if let (Some(media_session), None) = (media_session, state.media_session) {
                            warn!("set media session");
                            state.media_session = Some(media_session);
                        }
                        if let (Some(connection), false) = (state.media_connection(), was_connected) {
                            warn!("sending connected message");
                            let _ = tx.unbounded_send(Status::Connected {
                                transport: connection.transport.to_owned(),
                                session: connection.session.to_owned(),
                                media_session: connection.media_session,
                            });
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

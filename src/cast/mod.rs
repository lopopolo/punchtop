use byteorder::{BigEndian, ByteOrder};
use bytes::{Buf, BufMut, BytesMut, IntoBuf};
use std::io;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use futures::prelude::*;
use futures::sync::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
use futures::{future, Future};
use tokio::codec::{Decoder, Encoder, Framed};
use tokio::net::TcpStream;
use tokio::timer::Interval;
use tokio_tls::{TlsConnector, TlsStream};

mod message;
mod payload;
mod proto;
mod provider;

use self::message::namespace::*;
use self::payload::*;
pub use self::provider::*;

const CAST_MESSAGE_HEADER_LENGTH: usize = 4;
const DEFAULT_MEDIA_RECEIVER_APP_ID: &str = "CC1AD845";

#[derive(Debug)]
pub enum ChannelMessage {
    Connection(connection::Payload),
    Heartbeat(heartbeat::Payload),
    Media(media::Payload),
    Receiver(receiver::Payload),
}

#[derive(Debug)]
pub struct Channel<T, R> {
    pub tx: UnboundedSender<T>,
    pub rx: UnboundedReceiver<R>,
}

#[derive(Debug)]
pub struct Chromecast {
    pub chan: Channel<Command, Status>,
    pub session_id: Option<String>,
    pub media_session_id: Option<i32>,
}

impl Chromecast {
    pub fn poll_receiver_status(&self) {
        let _ = self.chan.tx.unbounded_send(Command::ReceiverStatus);
    }

    pub fn launch_app(&self) {
        let launch = Command::Launch(DEFAULT_MEDIA_RECEIVER_APP_ID.to_owned());
        let _ = self
            .chan
            .tx
            .unbounded_send(Command::Connect)
            .and_then(|_| self.chan.tx.unbounded_send(launch));
    }

    pub fn load(&self, media: Media) {
        if let Some(ref session_id) = self.session_id {
            let _ = self
                .chan
                .tx
                .unbounded_send(Command::Load(session_id.to_owned(), media));
        }
    }

    pub fn play(&self) {
        if let Some(media_session_id) = self.media_session_id {
            let _ = self.chan.tx.unbounded_send(Command::Play(media_session_id));
        }
    }

    pub fn stop(&self) {
        let _ = self
            .chan
            .tx
            .unbounded_send(Command::Stop(DEFAULT_MEDIA_RECEIVER_APP_ID.to_owned()));
    }

    pub fn close(&self) {
        let _ = self.chan.tx.unbounded_send(Command::Close);
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

pub fn connect(addr: SocketAddr, rt: &mut tokio::runtime::Runtime) -> Chromecast {
    let (command_tx, command_rx) = unbounded();
    let (status_tx, status_rx) = unbounded();

    let cast = Chromecast {
        chan: Channel {
            tx: command_tx.clone(),
            rx: status_rx,
        },
        session_id: None,
        media_session_id: None,
    };
    let init = tls_connect(addr).map(move |socket| {
        info!("TLS connection established");
        let (sink, source) = Framed::new(socket, CastMessageCodec::new()).split();
        tokio::spawn(reader(source, status_tx, command_tx.clone()));
        tokio::spawn(writer(sink, command_rx));
        tokio::spawn(heartbeat(command_tx.clone()));
    });
    rt.spawn(init.map_err(|_| ()));
    cast
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
        }
        ChannelMessage::Receiver(message) => match message {
            receiver::Payload::ReceiverStatus { status, .. } => {
                debug!("Got receiver stauts: {:?}", status);
                let session_id = status
                    .applications
                    .iter()
                    .find(|app| app.app_id == DEFAULT_MEDIA_RECEIVER_APP_ID)
                    .map(|app| app.session_id.to_owned());
                if let Some(session_id) = session_id {
                    let _ = tx.unbounded_send(Status::Connected(session_id));
                }
            }
            _ => {}
        },
        ChannelMessage::Media(message) => match message {
            media::Payload::MediaStatus { status, .. } => {
                debug!("Got media status");
                let media_session_id = status.first().map(|status| status.media_session_id);
                if let Some(media_session_id) = media_session_id {
                    let _ = tx.unbounded_send(Status::MediaConnected(media_session_id));
                }
            }
            _ => {}
        },
        payload => warn!("Got unknown payload: {:?}", payload),
    }
}

enum DecodeState {
    Header,
    Payload(usize),
}

struct CastMessageCodec {
    state: DecodeState,
    decode_counter: Arc<AtomicUsize>,
    encode_counter: Arc<AtomicUsize>,
}

impl CastMessageCodec {
    fn new() -> Self {
        CastMessageCodec {
            state: DecodeState::Header,
            decode_counter: Arc::new(AtomicUsize::new(0)),
            encode_counter: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl Encoder for CastMessageCodec {
    type Item = Command;
    type Error = io::Error;

    fn encode(&mut self, item: Self::Item, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let counter = self.encode_counter.fetch_add(1usize, Ordering::SeqCst) as i32;
        debug!(
            "CastMessageCodec encode-attempt={} command={:?}",
            counter, item
        );
        let message = match item {
            Command::Close => message::connection::close(),
            Command::Connect => message::connection::connect(),
            Command::Heartbeat => message::heartbeat::ping(),
            Command::Launch(ref app_id) => message::receiver::launch(counter, app_id),
            Command::Load(session_id, media) => message::media::load(counter, &session_id, media),
            Command::MediaStatus(_) => unimplemented!(),
            Command::Pause => unimplemented!(),
            Command::Play(media_session_id) => message::media::play(counter, media_session_id),
            Command::ReceiverStatus => message::receiver::status(counter),
            Command::Seek(_) => unimplemented!(),
            Command::Stop(ref session_id) => message::receiver::stop(counter, session_id),
            Command::Volume(_, _) => unimplemented!(),
        };

        let message = message.map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;
        let mut buf = Vec::new();
        message::encode(message, &mut buf)
            .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;

        // Cast wire protocol is a 4-byte big endian length-prefixed protobuf.
        let header = &mut [0; 4];
        BigEndian::write_u32(header, buf.len() as u32);

        dst.reserve(CAST_MESSAGE_HEADER_LENGTH + buf.len());
        dst.put_slice(header);
        dst.put_slice(&buf);
        Ok(())
    }
}

impl CastMessageCodec {
    fn decode_header(&mut self, src: &mut BytesMut) -> Option<usize> {
        // Cast wire protocol is a 4-byte big endian length-prefixed protobuf.
        // At least 4 bytes are required to decode the next frame.
        if src.len() < CAST_MESSAGE_HEADER_LENGTH {
            return None;
        }
        let header = src.split_to(4);
        let length = {
            let mut header = header.into_buf();
            header.get_u32_be() as usize
        };
        src.reserve(length);
        Some(length)
    }

    fn decode_payload(&self, n: usize, src: &mut BytesMut) -> Option<BytesMut> {
        if src.len() < n {
            return None;
        }
        Some(src.split_to(n))
    }
}

impl Decoder for CastMessageCodec {
    type Item = ChannelMessage;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let counter = self.decode_counter.fetch_add(1usize, Ordering::SeqCst) as i32;
        debug!("Decoding message {}", counter);
        let n = match self.state {
            DecodeState::Header => match self.decode_header(src) {
                Some(n) => {
                    self.state = DecodeState::Payload(n);
                    n
                }
                None => return Ok(None),
            },
            DecodeState::Payload(n) => n,
        };
        match self.decode_payload(n, src) {
            Some(mut src) => {
                self.state = DecodeState::Header;
                src.reserve(CAST_MESSAGE_HEADER_LENGTH);
                let message = protobuf::parse_from_bytes::<proto::CastMessage>(&src)
                    .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;
                match message.get_namespace() {
                    CONNECTION => {
                        serde_json::from_str::<connection::Payload>(message.get_payload_utf8())
                            .map_err(|err| io::Error::new(io::ErrorKind::Other, err))
                            .map(ChannelMessage::Connection)
                            .map(Some)
                    }
                    HEARTBEAT => {
                        serde_json::from_str::<heartbeat::Payload>(message.get_payload_utf8())
                            .map_err(|err| io::Error::new(io::ErrorKind::Other, err))
                            .map(ChannelMessage::Heartbeat)
                            .map(Some)
                    }
                    MEDIA => serde_json::from_str::<media::Payload>(message.get_payload_utf8())
                        .map_err(|err| io::Error::new(io::ErrorKind::Other, err))
                        .map(ChannelMessage::Media)
                        .map(Some),
                    RECEIVER => {
                        serde_json::from_str::<receiver::Payload>(message.get_payload_utf8())
                            .map_err(|err| io::Error::new(io::ErrorKind::Other, err))
                            .map(ChannelMessage::Receiver)
                            .map(Some)
                    }
                    channel => {
                        warn!("Received message on unknown channel: {}", channel);
                        Err(io::Error::new(
                            io::ErrorKind::Other,
                            Error::UnknownChannel(channel.to_owned()),
                        ))
                    }
                }
            }
            None => Ok(None),
        }
    }
}

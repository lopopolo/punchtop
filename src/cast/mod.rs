use byteorder::{BigEndian, ByteOrder};
use bytes::{Buf, BufMut, BytesMut, IntoBuf};
use std::error;
use std::fmt;
use std::io;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use futures::sink::Sink;
use futures::sync::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
use futures::{Future, Stream};
use native_tls::TlsConnector;
use tokio::codec::{Decoder, Encoder, Framed};
use tokio::net::TcpStream;
use tokio::runtime::Runtime;
use tokio::timer::Interval;
use url::Url;

mod payload;
mod proto;

use self::payload::*;

const CAST_MESSAGE_HEADER_LENGTH: usize = 4;
const DEFAULT_MEDIA_RECEIVER_APP_ID: &str = "CC1AD845";

#[derive(Clone, Debug)]
pub struct Media {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub url: Url,
    pub cover: Option<Image>,
}

impl fmt::Display for Media {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut parts = Vec::new();
        if let Some(ref artist) = self.artist {
            parts.push(artist.clone());
        }
        if let Some(ref title) = self.title {
            parts.push(title.clone());
        }
        if let Some(ref album) = self.album {
            parts.push(album.clone());
        }
        write!(f, "{}", parts.join(" -- "))
    }
}

#[derive(Clone, Debug)]
pub struct Image {
    pub url: Url,
    pub dimensions: (u32, u32),
}

#[derive(Debug)]
pub struct Channel<T, R> {
    pub tx: UnboundedSender<T>,
    pub rx: UnboundedReceiver<R>,
}

#[derive(Debug)]
pub enum ChannelMessage {
    Connection(connection::Payload),
    Heartbeat(heartbeat::Payload),
    Media(media::Payload),
    Receiver(receiver::Payload),
}

#[derive(Debug)]
pub enum Error {
    Connect,
    UnknownChannel(String),
}

impl error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::Connect => write!(f, "Failed to connect to Chromecast"),
            Error::UnknownChannel(ref channel) => write!(f, "Message received on unknown channel {:?}", channel)
        }
    }
}

#[derive(Debug)]
pub enum Command {
    Close,
    Connect,
    Heartbeat,
    Launch(String),
    Load(Media),
    MediaStatus(String),
    Pause,
    Play,
    ReceiverStatus,
    Seek(f32),
    Stop(String),
    Volume(f32, bool),
}

#[derive(Debug)]
pub enum Status {
    Connected,
    Media,
    LoadCancelled,
    LoadFailed,
    InvalidPlayerState,
    InvalidRequest,
}

#[derive(Debug)]
pub struct Chromecast {
    chan: Channel<Command, Status>,
}

impl Chromecast {
    pub fn poll_receiver_status(&self) {
        let _ = self.chan.tx.unbounded_send(Command::ReceiverStatus);
    }

    pub fn play(&self, media: Media) {
        let _ = self.chan.tx.unbounded_send(Command::Load(media));
    }

    pub fn stop(&self, app_id: &str) {
        let _ = self
            .chan
            .tx
            .unbounded_send(Command::Stop(app_id.to_owned()));
    }

    pub fn close(&self) {
        let _ = self.chan.tx.unbounded_send(Command::Close);
    }
}

pub fn connect(rt: &mut Runtime, addr: SocketAddr) -> Result<Chromecast, Error> {
    let socket = TcpStream::connect(&addr);
    let cx = TlsConnector::builder()
        .danger_accept_invalid_hostnames(true)
        .danger_accept_invalid_certs(true)
        .build()
        .map(tokio_tls::TlsConnector::from)
        .map_err(|_| Error::Connect)?;

    let (command_tx, command_rx) = unbounded();
    let (status_tx, status_rx) = unbounded();
    let command = command_tx.clone();
    let heartbeat = command_tx.clone();

    let cast = Chromecast {
        chan: Channel {
            tx: command_tx,
            rx: status_rx,
        },
    };

    let connect = socket
        .and_then(move |socket| {
            info!("Establishing TLS connection with Chromecast");
            cx.connect(&format!("{}", addr.ip()), socket)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
        })
        .map(move |socket| {
            info!("Chomecast connect successful");
            let (sink, source) = Framed::new(socket, CastMessageCodec::new()).split();
            let rx = source
                .for_each(move |message| {
                    let status = status_tx.clone();
                    let command = command.clone();
                    Ok(read(message, status, command))
                })
                .map(|_| ())
                .map_err(|err| {
                    warn!(
                        "Error running future Chromecast TLS payload reader: {:?}",
                        err
                    )
                });
            let tx = command_rx
                .forward(sink.sink_map_err(|_| ()))
                .map(|_| ())
                .map_err(|err| {
                    warn!(
                        "Error running future Chromecast receiver channel: {:?}",
                        err
                    )
                });
            let heartbeat = Interval::new_interval(Duration::new(5, 0))
                .map(|_| Command::Heartbeat)
                .map_err(|_| ())
                .forward(heartbeat.sink_map_err(|_| ()))
                .map(|_| ())
                .map_err(|err| {
                    warn!(
                        "Error running future Chromecast heartbeat channel: {:?}",
                        err
                    )
                });
            tokio::spawn(rx);
            tokio::spawn(tx);
            tokio::spawn(heartbeat);
        })
        .map(|_| ())
        .map_err(|err| warn!("Error running future Chromecast connect: {:?}", err));

    rt.spawn(connect);
    cast.chan
        .tx
        .unbounded_send(Command::Connect)
        .and_then(|_| {
            cast.chan
                .tx
                .unbounded_send(Command::Launch(DEFAULT_MEDIA_RECEIVER_APP_ID.to_owned()))
        })
        .map(|_| cast)
        .map_err(|_| Error::Connect)
}

fn read(message: ChannelMessage, tx: UnboundedSender<Status>, command: UnboundedSender<Command>) {
    debug!("Message on receiver channel");
    match message {
        ChannelMessage::Heartbeat(_) => debug!("Got heartbeat"),
        ChannelMessage::Receiver(message) => match message {
            receiver::Payload::ReceiverStatus { request_id, status } => {
                debug!(
                    "Got reciver status request_id={} status={:?}",
                    request_id, status
                );
            }
            _ => {}
        }
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
            Command::Close => message::close(),
            Command::Connect => message::connect(),
            Command::Heartbeat => message::ping(),
            Command::Launch(ref app_id) => message::launch(counter, app_id),
            Command::Load(_) => unimplemented!(),
            Command::MediaStatus(_) => unimplemented!(),
            Command::Pause => unimplemented!(),
            Command::Play => unimplemented!(),
            Command::ReceiverStatus => message::receiver_status(counter),
            Command::Seek(_) => unimplemented!(),
            Command::Stop(ref session_id) => message::stop(counter, session_id),
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
                    connection::NAMESPACE => serde_json::from_str::<connection::Payload>(message.get_payload_utf8())
                        .map_err(|err| io::Error::new(io::ErrorKind::Other, err))
                        .map(ChannelMessage::Connection)
                        .map(Some),
                    heartbeat::NAMESPACE => serde_json::from_str::<heartbeat::Payload>(message.get_payload_utf8())
                        .map_err(|err| io::Error::new(io::ErrorKind::Other, err))
                        .map(ChannelMessage::Heartbeat)
                        .map(Some),
                    media::NAMESPACE => serde_json::from_str::<media::Payload>(message.get_payload_utf8())
                        .map_err(|err| io::Error::new(io::ErrorKind::Other, err))
                        .map(ChannelMessage::Media)
                        .map(Some),
                    receiver::NAMESPACE => serde_json::from_str::<receiver::Payload>(message.get_payload_utf8())
                        .map_err(|err| io::Error::new(io::ErrorKind::Other, err))
                        .map(ChannelMessage::Receiver)
                        .map(Some),
                    channel => Err(io::Error::new(io::ErrorKind::Other, Error::UnknownChannel(channel.to_owned()))),
                }
            }
            None => Ok(None),
        }
    }
}

mod message {
    use protobuf::{CodedOutputStream, ProtobufResult};

    use super::payload::*;
    use super::proto;

    const DEFAULT_SENDER_ID: &str = "sender-0";
    const DEFAULT_DESTINATION_ID: &str = "receiver-0";

    pub fn encode(msg: impl protobuf::Message, buf: &mut Vec<u8>) -> ProtobufResult<()> {
        let mut output = CodedOutputStream::new(buf);
        msg.write_to(&mut output)?;
        output.flush()
    }

    pub fn connect() -> Result<proto::CastMessage, serde_json::Error> {
        let payload = serde_json::to_string(&connection::Payload::Connect)?;
        Ok(message(connection::NAMESPACE, payload))
    }

    pub fn close() -> Result<proto::CastMessage, serde_json::Error> {
        let payload = serde_json::to_string(&connection::Payload::Close)?;
        Ok(message(connection::NAMESPACE, payload))
    }

    pub fn ping() -> Result<proto::CastMessage, serde_json::Error> {
        let namespace = "urn:x-cast:com.google.cast.tp.heartbeat";
        let payload = serde_json::to_string(&heartbeat::Payload::Ping)?;
        Ok(message(heartbeat::NAMESPACE, payload))
    }

    pub fn pong() -> Result<proto::CastMessage, serde_json::Error> {
        let payload = serde_json::to_string(&heartbeat::Payload::Pong)?;
        Ok(message(heartbeat::NAMESPACE, payload))
    }

    pub fn launch(request_id: i32, app_id: &str) -> Result<proto::CastMessage, serde_json::Error> {
        let payload = serde_json::to_string(&receiver::Payload::Launch {
            request_id,
            app_id: app_id.to_owned(),
        })?;
        Ok(message(receiver::NAMESPACE, payload))
    }

    pub fn stop(
        request_id: i32,
        session_id: &str,
    ) -> Result<proto::CastMessage, serde_json::Error> {
        let payload = serde_json::to_string(&receiver::Payload::Stop {
            request_id,
            session_id: session_id.to_owned(),
        })?;
        Ok(message(receiver::NAMESPACE, payload))
    }

    pub fn receiver_status(request_id: i32) -> Result<proto::CastMessage, serde_json::Error> {
        let namespace = "urn:x-cast:com.google.cast.receiver";
        let payload = serde_json::to_string(&receiver::Payload::GetStatus { request_id })?;
        Ok(message(receiver::NAMESPACE, payload))
    }

    fn message(namespace: &str, payload: String) -> proto::CastMessage {
        let mut msg = proto::CastMessage::new();
        msg.set_payload_type(proto::CastMessage_PayloadType::STRING);
        msg.set_protocol_version(proto::CastMessage_ProtocolVersion::CASTV2_1_0);
        msg.set_namespace(namespace.to_owned());
        msg.set_source_id(DEFAULT_SENDER_ID.to_owned());
        msg.set_destination_id(DEFAULT_DESTINATION_ID.to_owned());
        msg.set_payload_utf8(payload);
        msg
    }
}

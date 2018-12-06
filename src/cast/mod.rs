use byteorder::{BigEndian, ReadBytesExt};
use bytes::{Buf, Bytes, BytesMut, IntoBuf};
use std::fmt;
use std::io::{Cursor, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering, ATOMIC_USIZE_INIT};
use std::thread;

use crossbeam_channel::{self, after, unbounded, Sender, Receiver};
use native_tls::TlsConnector;
use futures::Future;
use tokio;
use tokio::net::TcpStream;
use tokio::runtime::Runtime;
use tokio_io::AsyncRead;
use tokio_io::codec::{Decoder, Encoder};
use tokio_tls::TlsConnector as TokioTlsConnector;
use serde_json::Value as Json;
use url::Url;

mod proto;

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
        let parts: Vec<String> = vec![self.artist, self.title, self.album]
            .into_iter()
            .filter_map(|part| part)
            .collect();
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
    pub tx: Sender<T>,
    pub rx: Receiver<R>,
}

#[derive(Debug)]
pub enum Error {
    Connect,
}

#[derive(Debug)]
pub enum Command {
    Connect,
    Load(Media),
}

#[derive(Debug)]
pub enum Status {
    Connected,
    MediaStatus,
    LoadCancelled,
    LoadFailed,
    InvalidPlayerState,
    InvalidRequest,
}

#[derive(Clone, Copy, Debug)]
pub struct ReceiverVolume {
    pub level: f64,
    pub muted: bool,
}

#[derive(Debug)]
pub struct Chromecast {
    message_counter: AtomicUsize,
    stream: Arc<SslStream<TcpStream>>,
    chan: Channel<Command, Status>,
}

impl Chromecast {
    pub fn connect(addr: SocketAddr) -> Result<Self, Error> {
        let (command_tx, command_rx) = unbounded();
        let (status_tx, status_rx) = unbounded();
        let chan = Channel { tx: command_tx, rx: status_rx };
        let cast = Chromecast { message_counter: ATOMIC_USIZE_INIT, stream, chan };

        let socket = TcpStream::connect(&addr);
        let cx = TlsConnector::builder().build().or(Error::Connect)?;
        let cx = TokioTlsConnector::from(cx);

        let connect = socket.and_then(move |socket| {
            cx.connect("www.rust-lang.org", socket).map_err(|e| {
                io::Error::new(io::ErrorKind::Other, e)
            })
        })
        .and_then(move |socket| {
            let (read, write) = socket.framed(CastMessageCodec).split();
            let chan = Channel { tx: status_tx, rx: command_rx };
            tokio::spawn(move || { Chromecast::read(chan, read) });
            let msg = message::connect();
            write.write(msg);
        });
        tokio::run(connect);
        Ok(cast)
    }

    fn read(chan: Channel<Status, Command>, mut stream: TcpStream) -> () {
        let mut stream = Arc::make_mut(&mut stream);
        loop {
            let mut buffer: [u8; 4] = [0; 4];
            let _ = stream.read_exact(&mut buffer);

            if let Ok(length) = Cursor::new(buffer).read_u32::<BigEndian>() {
                let mut buffer: Vec<u8> = Vec::with_capacity(length as usize);
                let mut reader = stream.take(u64::from(length));
                let _ = reader.read_to_end(&mut buffer);
                let mut cursor = Cursor::new(buffer);

                let message = match protobuf::parse_from_reader::<proto::CastMessage>(&mut cursor) {
                    Ok(message) => message,
                    _ => continue,
                };
                let reply = match serde_json::from_str::<Json>(message.get_payload_utf8()) {
                    Ok(reply) => reply,
                    _ => continue,
                };
                let message_type = message::digs(&reply, &vec!["type"]);
                let message_type: &str = message_type.deref().unwrap_or("");
                match message.get_namespace() {
                    "urn:x-cast:com.google.cast.tp.heartbeat" => {
                        let msg = message::pong();
                        if let Ok(bytes) = message::encode(msg) {
                            stream.write(&bytes);
                        }
                    }
                    "urn:x-cast:com.google.cast.tp.connection" => {
                        match message_type {
                            "CLOSE" => {
                                // debug!("Chromecast connection close");
                                break;
                            }
                            _ => {}
                        }
                    }
                    "urn:x-cast:com.google.cast.media" => {
                        let _ = match message_type {
                            "MEDIA_STATUS" => {Ok(())}
                            "LOAD_CANCELLED" => chan.tx.try_send(Status::LoadCancelled),
                            "LOAD_FAILED" => chan.tx.try_send(Status::LoadFailed),
                            "INVALID_PLAYER_STATE" => chan.tx.try_send(Status::InvalidPlayerState),
                            "INVALID_REQUEST" => chan.tx.try_send(Status::InvalidRequest),
                            _ => Ok(()),
                        };
                    }
                    "urn:x-cast:com.google.cast.receiver" => {
                        match message_type {
                            "RECEIVER_STATUS" => {
                                let level = message::digf(&reply, &vec!["status", "volume", "level"]);
                                let muted = message::digb(&reply, &vec!["status", "volume", "muted"]);
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

struct CastMessageCodec;

impl Encoder for CastMessageCodec {
    type Item = proto::CastMessage;
    type Error = protobuf::error::ProtobufError;

    fn encode(&mut self, item: Self::Item, dst: &mut BytesMut) -> Result<(), Self::Error> {
        match message::encode(item) {
            Ok(bytes) => {
                dst.put_slice(&bytes);
                Ok(())
            }
            Err(err) => Err(err),
        }
    }
}

impl Decoder for CastMessageCodec {
    type Item = proto::CastMessage;
    type Error = protobuf::error::ProtobufError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        // Protobuf length is the first 4 bytes of the message. decode requires
        // at least 4 bytes to attempt to process the message.
        if src.len() < 4 {
            return Ok(None);
        }
        let header = src.split_to(4);
        let length = BigEndian::read_u32(header.as_ref());
        if src.len() < length {
            return Ok(None);
        }
        let message = src.truncate(length);
        protobuf::parse_from_bytes::<proto::CastMessage>(&message)
            .map(|msg| Some(msg))
    }
}

mod message {
    use protobuf::{CodedOutputStream, Message, ProtobufResult};
    use serde_json::Value as Json;

    use super::proto;

    const DEFAULT_SENDER_ID: &str = "sender-0";
    const DEFAULT_DESTINATION_ID: &str = "receiver-0";

    pub fn digs<'a>(obj: &'a Json, keys: &[&str]) -> Option<String> {
        dig(obj, keys).and_then(|obj| obj.as_str()).map(String::from)
    }

    pub fn digf<'a>(obj: &'a Json, keys: &[&str]) -> Option<f64> {
        dig(obj, keys).and_then(|obj| obj.as_f64())
    }

    pub fn digb<'a>(obj: &'a Json, keys: &[&str]) -> Option<bool> {
        dig(obj, keys).and_then(|obj| obj.as_bool())
    }

    pub fn dig<'a>(obj: &'a Json, keys: &[&str]) -> Option<Json> {
        let mut curr = obj;
        for key in keys {
            curr = match curr.as_object().and_then(|object| object.get(key.to_owned())) {
                Some(child) => child,
                None => return None,
            };
        }
        Some(curr.clone())
    }

    pub fn encode(msg: proto::CastMessage) -> ProtobufResult<Vec<u8>> {
        let mut bytes = Vec::new();
        let mut output = CodedOutputStream::new(&mut bytes);
        msg.write_to(&mut output)?;
        output.flush();
        Ok(bytes)
    }

    pub fn connect() -> proto::CastMessage {
        let namespace = "urn:x-cast:com.google.cast.tp.connection";
        let payload = json!({"type": "CONNECT", "origin": {}});
        message(namespace, payload)
    }

    pub fn pong() -> proto::CastMessage {
        let namespace = "urn:x-cast:com.google.cast.tp.heartbeat";
        let payload = json!({"type": "PONG"});
        message(namespace, payload)
    }

    fn message(namespace: &str, payload: Json) -> proto::CastMessage {
        let mut msg = proto::CastMessage::new();
        msg.set_payload_type(proto::CastMessage_PayloadType::STRING);
        msg.set_protocol_version(proto::CastMessage_ProtocolVersion::CASTV2_1_0);
        msg.set_namespace(namespace.to_owned());
        msg.set_source_id(DEFAULT_SENDER_ID.to_owned());
        msg.set_destination_id(DEFAULT_DESTINATION_ID.to_owned());
        msg.set_payload_utf8(format!("{}", payload));
        msg
    }
}

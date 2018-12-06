use bytes::{Buf, BufMut, BytesMut, IntoBuf};
use std::fmt;
use std::io;
use std::io::{Read, Write};
use std::net::SocketAddr;
use std::sync::atomic::AtomicUsize;

use native_tls::TlsConnector;
use futures::{Future, Sink, Stream};
use futures::sync::mpsc::{unbounded, UnboundedSender, UnboundedReceiver};
use tokio;
use tokio::net::TcpStream;
use tokio_codec::Framed;
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
    pub tx: UnboundedSender<T>,
    pub rx: UnboundedReceiver<R>,
}

#[derive(Debug)]
pub enum Error {
    Connect,
}

#[derive(Debug)]
pub enum Command {
    Connect,
    Heartbeat,
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
    write: u8,
    chan: Channel<Command, Status>,
}

impl Chromecast {
    pub fn connect(addr: SocketAddr) -> Result<Self, Error> {
        let socket = TcpStream::connect(&addr);
        let cx = TlsConnector::builder().build().or(Err(Error::Connect))?;
        let cx = TokioTlsConnector::from(cx);

        let (command_tx, command_rx) = unbounded();
        let (status_tx, status_rx) = unbounded();

        let connect = socket.and_then(move |socket| {
            cx.connect("www.rust-lang.org", socket).map_err(|e| {
                io::Error::new(io::ErrorKind::Other, e)
            })
        })
        .and_then(move |socket| {
            let (mut write, read) = Framed::new(socket, CastMessageCodec).split();
            let chan = Channel { tx: status_tx, rx: command_rx };
            tokio::spawn(read.then(|message| {
                message.map(|message| Chromecast::read(message, status_tx, command_tx))
            }));

            command_tx.unbounded_send(Command::Connect);
            write.then(|write| write.send(message::connect()))
        });
        tokio::run(connect);
        Err(Error::Connect)
    }

    fn read(message: proto::CastMessage, tx: UnboundedSender<Status>, heartbeat: UnboundedSender<Command>) -> () {
        if let Ok(reply) = serde_json::from_str::<Json>(message.get_payload_utf8()) {
            let message_type = message::digs(&reply, &vec!["type"]);
            let message_type: &str = message_type.deref().unwrap_or("");
            match message.get_namespace() {
                "urn:x-cast:com.google.cast.tp.heartbeat" => {
                    heartbeat.unbounded_send(Command::Heartbeat);
                }
                "urn:x-cast:com.google.cast.tp.connection" => {
                    match message_type {
                        "CLOSE" => {
                            // debug!("Chromecast connection close");
                        }
                        _ => {}
                    }
                }
                "urn:x-cast:com.google.cast.media" => {
                    let _ = match message_type {
                        "MEDIA_STATUS" => {Ok(())}
                        "LOAD_CANCELLED" => tx.unbounded_send(Status::LoadCancelled),
                        "LOAD_FAILED" => tx.unbounded_send(Status::LoadFailed),
                        "INVALID_PLAYER_STATE" => tx.unbounded_send(Status::InvalidPlayerState),
                        "INVALID_REQUEST" => tx.unbounded_send(Status::InvalidRequest),
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
        let length = {
            let header = header.into_buf();
            header.get_u32_be() as usize
        };
        if src.len() < length {
            return Ok(None);
        }
        src.truncate(length);
        protobuf::parse_from_bytes::<proto::CastMessage>(&src)
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

use byteorder::{BigEndian, WriteBytesExt};
use bytes::{Buf, BufMut, BytesMut, IntoBuf};
use std::error;
use std::fmt;
use std::io;
use std::net::SocketAddr;
use std::sync::atomic::AtomicUsize;
use std::time::Duration;

use futures::sink::Sink;
use futures::sync::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
use futures::{future, Future, Stream};
use native_tls::TlsConnector;
use serde_json::Value as Json;
use tokio;
use tokio::net::TcpStream;
use tokio_codec::Framed;
use tokio_io::codec::{Decoder, Encoder};
use tokio_timer::Interval;
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
pub enum Error {
    Connect,
}

impl error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::Connect => write!(f, "Failed to connect to Chromecast"),
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
    Stop(String),
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
    pub fn connect(addr: SocketAddr) -> Result<Channel<Command, Status>, Error> {
        let socket = TcpStream::connect(&addr);
        let cx = TlsConnector::builder()
            .danger_accept_invalid_hostnames(true)
            .danger_accept_invalid_certs(true)
            .build()
            .map(tokio_tls::TlsConnector::from)
            .map_err(|_| Error::Connect)?;

        let (command_tx, command_rx) = unbounded();
        let (status_tx, status_rx) = unbounded();
        let heartbeat = command_tx.clone();

        let connect = socket
            .and_then(move |socket| {
                println!("Establishing TLS connection with Chromecast");
                cx.connect(&format!("{}", addr.ip()), socket)
                    .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
            })
            .map(move |socket| {
                println!("Chomecast connect successful");
                let (write, read) = Framed::new(socket, CastMessageCodec).split();
                let rx = read
                    .then(move |message| {
                        let status = status_tx.clone();
                        message.map(|msg| Chromecast::read(msg, status))
                    })
                    .into_future()
                    .map(|_| ())
                    .map_err(|_| ());
                let tx = command_rx
                    .forward(write.sink_map_err(|err| println!("write err: {:?}", err)))
                    .map(|_| ())
                    .map_err(|err| println!("write err: {:?}", err));
                let heartbeat = Interval::new_interval(Duration::new(5, 0))
                    .for_each(move |_| {
                        println!("Sending heartbeat PING");
                        let r = heartbeat.unbounded_send(Command::Heartbeat);
                        println!("heartbeat send: {:?}, closed? {:?}", r, heartbeat.is_closed());
                        future::ok(())
                    })
                    .map_err(|_| ());
                tokio::spawn(rx);
                tokio::spawn(tx);
                tokio::spawn(heartbeat);
            })
            .map(|_| ())
            .map_err(|err| println!("Chromecast connect err: {:?}", err));

        println!("connect command: {:?}", command_tx.unbounded_send(Command::Connect));
        // let _ = command_tx.unbounded_send(Command::Launch("CC1AD845".to_owned()));
        tokio::run(connect);

        Ok(Channel {
            tx: command_tx,
            rx: status_rx,
        })
    }

    fn read(
        message: proto::CastMessage,
        tx: UnboundedSender<Status>,
    ) {
        if let Ok(reply) = serde_json::from_str::<Json>(message.get_payload_utf8()) {
            let message_type = message::digs(&reply, &vec!["type"]);
            let message_type: &str = message_type.deref().unwrap_or("");
            println!("{:?} {:?}", message.get_namespace(), message_type);
            match message.get_namespace() {
                "urn:x-cast:com.google.cast.tp.heartbeat" => {
                    println!("Got heartbeat");
                    // let _ = command.unbounded_send(Command::Heartbeat);
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
                        "MEDIA_STATUS" => Ok(()),
                        "LOAD_CANCELLED" => tx.unbounded_send(Status::LoadCancelled),
                        "LOAD_FAILED" => tx.unbounded_send(Status::LoadFailed),
                        "INVALID_PLAYER_STATE" => tx.unbounded_send(Status::InvalidPlayerState),
                        "INVALID_REQUEST" => tx.unbounded_send(Status::InvalidRequest),
                        _ => Ok(()),
                    };
                }
                "urn:x-cast:com.google.cast.receiver" => match message_type {
                    "RECEIVER_STATUS" => {
                        let level = message::digf(&reply, &vec!["status", "volume", "level"]);
                        let muted = message::digb(&reply, &vec!["status", "volume", "muted"]);
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }
}

struct CastMessageCodec;

impl Encoder for CastMessageCodec {
    type Item = Command;
    type Error = protobuf::error::ProtobufError;

    fn encode(&mut self, item: Self::Item, dst: &mut BytesMut) -> Result<(), Self::Error> {
        println!("Encoding cast command: {:?}", item);
        let message = match item {
            Command::Connect => message::connect(),
            Command::Close => message::close(),
            Command::Heartbeat => message::ping(),
            Command::Launch(ref app_id) => message::launch(app_id),
            Command::Load(_) => message::connect(),
            Command::Stop(ref session_id) => message::stop(session_id),
        };
        let mut buf = Vec::new();
        match message::encode(message, &mut buf) {
            Ok(()) => {
                let header = {
                    let mut len = vec![];
                    len.write_u32::<BigEndian>(buf.len() as u32).unwrap();
                    len
                };

                dst.put_slice(&header);
                dst.put_slice(&buf);
                Ok(())
            }
            Err(err) => {println!("encoder error: {:?}", err); Err(err)}
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
            let mut header = header.into_buf();
            header.get_u32_be() as usize
        };
        if src.len() < length {
            return Ok(None);
        }
        src.truncate(length);
        protobuf::parse_from_bytes::<proto::CastMessage>(&src).map(|msg| Some(msg))
    }
}

mod message {
    use protobuf::{CodedOutputStream, ProtobufResult};
    use serde_json::Value as Json;

    use super::proto;

    const DEFAULT_SENDER_ID: &str = "sender-0";
    const DEFAULT_DESTINATION_ID: &str = "receiver-0";

    pub fn digs<'a>(obj: &'a Json, keys: &[&str]) -> Option<String> {
        dig(obj, keys).and_then(|obj| obj.as_str().map(String::from))
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
            let elem = curr
                .as_object()
                .and_then(|object| object.get(key.to_owned()));
            curr = match elem {
                Some(child) => child,
                None => return None,
            };
        }
        Some(curr.clone())
    }

    pub fn encode(msg: impl protobuf::Message, buf: &mut Vec<u8>) -> ProtobufResult<()> {
        let mut output = CodedOutputStream::new(buf);
        msg.write_to(&mut output)?;
        output.flush()
    }

    pub fn connect() -> proto::CastMessage {
        let namespace = "urn:x-cast:com.google.cast.tp.connection";
        let payload = json!({"type": "CONNECT"});
        message(namespace, payload)
    }

    pub fn close() -> proto::CastMessage {
        let namespace = "urn:x-cast:com.google.cast.tp.connection";
        let payload = json!({"type": "CLOSE"});
        message(namespace, payload)
    }

    pub fn ping() -> proto::CastMessage {
        let namespace = "urn:x-cast:com.google.cast.tp.heartbeat";
        let payload = json!({"type": "PING"});
        message(namespace, payload)
    }

    pub fn pong() -> proto::CastMessage {
        let namespace = "urn:x-cast:com.google.cast.tp.heartbeat";
        let payload = json!({"type": "PONG"});
        message(namespace, payload)
    }

    pub fn launch(app_id: &str) -> proto::CastMessage {
        let namespace = "urn:x-cast:com.google.cast.receiver";
        let payload = json!({ "type": "LAUNCH", "appId": app_id });
        message(namespace, payload)
    }

    pub fn stop(session_id: &str) -> proto::CastMessage {
        let namespace = "urn:x-cast:com.google.cast.receiver";
        let payload = json!({ "type": "STOP", "sessionId": session_id });
        message(namespace, payload)
    }

    pub fn status() -> proto::CastMessage {
        let namespace = "urn:x-cast:com.google.cast.receiver";
        let payload = json!({ "type": "GET_STATUS" });
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

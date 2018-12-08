use byteorder::{BigEndian, ByteOrder};
use bytes::{Buf, BufMut, BytesMut, IntoBuf};
use std::error;
use std::fmt;
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use futures::sink::Sink;
use futures::sync::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
use futures::{Future, Stream};
use native_tls::TlsConnector;
use rand::{thread_rng, Rng};
use tokio::codec::{Decoder, Encoder, Framed};
use tokio::net::TcpStream;
use tokio::timer::Interval;
use url::Url;

mod payload;
mod proto;

use self::payload::Payload;

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
    Launch(i64, String),
    Load(i64, Media),
    Pause(i64),
    Play(i64),
    Seek(i64, f32),
    Status(i64),
    Stop(i64, String),
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

#[derive(Clone, Copy, Debug)]
pub struct ReceiverVolume {
    pub level: f64,
    pub muted: bool,
}

#[derive(Debug)]
pub struct Chromecast {
    message_counter: Arc<AtomicUsize>,
    write: u8,
    chan: Channel<Command, Status>,
}

impl Chromecast {
    pub fn connect(addr: SocketAddr) -> Result<Channel<Command, Status>, Error> {
        //let req_id = Arc::new(AtomicUsize::new(thread_rng().gen_range(0 as usize, 1 << 20 as usize)));
        let req_id = Arc::new(AtomicUsize::new(0));
        let send_req_id = Arc::clone(&req_id);
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
                        let command = command.clone();
                        let req_id = Arc::clone(&send_req_id);
                        message.map(|msg| Chromecast::read(msg, status, command, req_id))
                    })
                    .into_future();
                let tx = command_rx
                    .forward(write.sink_map_err(|_| ()));
                let heartbeat = Interval::new_interval(Duration::new(5, 0))
                    .map(|_| Command::Heartbeat)
                    .map_err(|_| ())
                    .forward(heartbeat.sink_map_err(|_| ()));
                tokio::spawn(rx.map(|_| ()).map_err(|_| println!("rx error")));
                tokio::spawn(tx.map(|_| ()).map_err(|e| println!("tx {:?}", e)));
                tokio::spawn(heartbeat.map(|_| ()).map_err(|e| println!("heartbeat {:?}", e)));
            })
            .map(|_| ())
            .map_err(|err| println!("Chromecast connect err: {:?}", err));

        println!("connect command: {:?}", command_tx.unbounded_send(Command::Connect));
        let _ = command_tx.unbounded_send(
            Command::Launch(req_id.fetch_add(1usize, Ordering::SeqCst) as i64,
            "CC1AD845".to_owned())
        );
        tokio::run(connect);

        Ok(Channel {
            tx: command_tx,
            rx: status_rx,
        })
    }

    fn read(
        message: Payload,
        tx: UnboundedSender<Status>,
        command: UnboundedSender<Command>,
        req_id: Arc<AtomicUsize>,
    ) {
        match message {
            Payload::Pong {} => {
                println!("Got PONG from receiver");
                println!("{:?}", command.unbounded_send(
                        Command::Status(
                            req_id.fetch_add(1usize, Ordering::SeqCst) as i64
                        )));
            }
            Payload::ReceiverStatus { request_id, status } => {
                println!("got status for req id: {}", request_id);
                println!("status: {:?}", status);
                println!("{:?}", command.unbounded_send(
                        Command::Status(
                            req_id.fetch_add(1usize, Ordering::SeqCst) as i64
                        )));
            }
            payload => println!("unknown payload: {:?}", payload),
        }
    }
}

struct CastMessageCodec;

impl Encoder for CastMessageCodec {
    type Item = Command;
    type Error = io::Error;

    fn encode(&mut self, item: Self::Item, dst: &mut BytesMut) -> Result<(), Self::Error> {
        println!("Encoding cast command: {:?}", item);
        let message = match item {
            Command::Close => message::close(),
            Command::Connect => message::connect(),
            Command::Heartbeat => message::ping(),
            Command::Launch(req_id, ref app_id) => message::launch(req_id, app_id),
            Command::Load(_, _) => unimplemented!(),
            Command::Pause(_) => unimplemented!(),
            Command::Play(_) => unimplemented!(),
            Command::Seek(_, _) => unimplemented!(),
            Command::Status(req_id) => message::status(req_id),
            Command::Stop(req_id, ref session_id) => message::stop(req_id, session_id),
        };
        let message = message.map_err(|err| {println!("msg gen error: {:?}", err); io::Error::new(io::ErrorKind::Other, err)})?;
        println!("message: {:?}", message);
        let mut buf = Vec::new();
        message::encode(message, &mut buf)
            .map_err(|err| {println!("msg gen error: {:?}", err); io::Error::new(io::ErrorKind::Other, err)})?;
        // Cast wire protocol is a 4-byte big endian length-prefixed protobuf.
        let header = &mut [0u8; 4];
        BigEndian::write_u32(header, buf.len() as u32);

        dst.put_slice(header);
        dst.put_slice(&buf);
        Ok(())
    }
}

impl Decoder for CastMessageCodec {
    type Item = Payload;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        // Cast wire protocol is a 4-byte big endian length-prefixed protobuf.
        // At least 4 bytes are required to decode the next frame.
        println!("decode attempt");
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
        // TODO: remove this excessive logging, or convert them to trace level logs.
        let message = protobuf::parse_from_bytes::<proto::CastMessage>(&src)
            .map_err(|err| {println!("decode error: {:?}", err); io::Error::new(io::ErrorKind::Other, err)})?;
        println!("payload: {:?}", message.get_payload_utf8());
        serde_json::from_str::<Payload>(message.get_payload_utf8())
            .map_err(|err| {println!("decode error: {:?}", err); io::Error::new(io::ErrorKind::Other, err)})
            .map(Some)
    }
}

mod message {
    use protobuf::{CodedOutputStream, ProtobufResult};

    use super::payload::Payload;
    use super::proto;

    const DEFAULT_SENDER_ID: &str = "sender-0";
    const DEFAULT_DESTINATION_ID: &str = "receiver-0";

    pub fn encode(msg: impl protobuf::Message, buf: &mut Vec<u8>) -> ProtobufResult<()> {
        let mut output = CodedOutputStream::new(buf);
        msg.write_to(&mut output)?;
        output.flush()
    }

    pub fn connect() -> Result<proto::CastMessage, serde_json::Error> {
        let namespace = "urn:x-cast:com.google.cast.tp.connection";
        let payload = serde_json::to_string(&Payload::Connect)?;
        Ok(message(namespace, payload))
    }

    pub fn close() -> Result<proto::CastMessage, serde_json::Error> {
        let namespace = "urn:x-cast:com.google.cast.tp.connection";
        let payload = serde_json::to_string(&Payload::Close)?;
        Ok(message(namespace, payload))
    }

    pub fn ping() -> Result<proto::CastMessage, serde_json::Error> {
        let namespace = "urn:x-cast:com.google.cast.tp.heartbeat";
        let payload = serde_json::to_string(&Payload::Ping)?;
        Ok(message(namespace, payload))
    }

    pub fn pong() -> Result<proto::CastMessage, serde_json::Error> {
        let namespace = "urn:x-cast:com.google.cast.tp.heartbeat";
        let payload = serde_json::to_string(&Payload::Pong)?;
        Ok(message(namespace, payload))
    }

    pub fn launch(request_id: i64, app_id: &str) -> Result<proto::CastMessage, serde_json::Error> {
        let namespace = "urn:x-cast:com.google.cast.receiver";
        let payload = serde_json::to_string(&Payload::Launch {
            request_id,
            app_id: app_id.to_owned()
        })?;
        Ok(message(namespace, payload))
    }

    pub fn stop(request_id: i64, session_id: &str) -> Result<proto::CastMessage, serde_json::Error> {
        let namespace = "urn:x-cast:com.google.cast.receiver";
        let payload = serde_json::to_string(&Payload::Stop {
            request_id,
            session_id: session_id.to_owned(),
        })?;
        Ok(message(namespace, payload))
    }

    pub fn status(request_id: i64) -> Result<proto::CastMessage, serde_json::Error> {
        let namespace = "urn:x-cast:com.google.cast.receiver";
        let payload = serde_json::to_string(&Payload::GetStatus { request_id })?;
        Ok(message(namespace, payload))
    }

    fn message(namespace: &str, payload: String) -> proto::CastMessage {
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

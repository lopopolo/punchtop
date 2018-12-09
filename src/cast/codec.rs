use std::io;
use std::sync::atomic::{AtomicUsize, Ordering};

use byteorder::{BigEndian, ByteOrder};
use bytes::{Buf, BufMut, BytesMut, IntoBuf};
use tokio::codec::{Decoder, Encoder};

use super::{message, ChannelMessage};
use super::message::namespace;
use super::payload::*;
use super::proto::CastMessage;
use super::provider::*;

const CAST_MESSAGE_HEADER_LENGTH: usize = 4;

/// `CastMessageCodec` decodes a length-prefixed protobuf. This enum represents
/// the phase of the decoding. Keep track of the decode phase to ensure the
/// decoder does not drop bytes from the `BytesMut`.
enum DecodeState {
    /// Waiting to read a u32 representing the size of the next protobuf.
    Header,
    /// Reading a protobuf with a given length.
    Payload(usize),
}

pub struct CastMessageCodec {
    state: DecodeState,
    req_id: AtomicUsize,
    decoded_frames: AtomicUsize,
    encoded_frames: AtomicUsize,
}

impl CastMessageCodec {
    pub fn new() -> Self {
        CastMessageCodec {
            state: DecodeState::Header,
            req_id: AtomicUsize::new(1),
            decoded_frames: AtomicUsize::new(0),
            encoded_frames: AtomicUsize::new(0),
        }
    }
}

impl Encoder for CastMessageCodec {
    type Item = Command;
    type Error = io::Error;

    fn encode(&mut self, item: Self::Item, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let req_id = self.req_id.fetch_add(1usize, Ordering::SeqCst) as i32;
        let encode_counter = self.encoded_frames.fetch_add(1usize, Ordering::SeqCst);
        trace!(
            "CastMessageCodec stream=encode frame-counter={} command={:?}",
            encode_counter, item
        );
        let message = match item {
            Command::Close(connect) => message::connection::close(&connect.transport),
            Command::Connect(connect) => message::connection::connect(&connect.transport),
            Command::Heartbeat => message::heartbeat::ping(),
            Command::Launch { app_id } => message::receiver::launch(req_id, &app_id),
            Command::Load { connect, media } =>
                message::media::load(req_id, &connect.session, &connect.transport, media),
            Command::MediaStatus(connect) => message::media::status(req_id, &connect.receiver.transport, connect.session),
            Command::Play(ref connect) if connect.session.is_some() =>
                message::media::play(req_id, &connect.receiver.transport, connect.session.unwrap()),
            Command::ReceiverStatus => message::receiver::status(req_id),
            _ => unimplemented!(),
        };

        let message = message.map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;
        let mut buf = Vec::new();
        message::encode(message, &mut buf)
            .map_err(|err| {warn!("encode err: {:?}", err); err})
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
        let message = match self.decode_payload(n, src) {
            Some(mut src) => {
                self.state = DecodeState::Header;
                src.reserve(CAST_MESSAGE_HEADER_LENGTH);
                let message = protobuf::parse_from_bytes::<CastMessage>(&src)
                    .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;
                trace!("decoded message with namespace: {}", message.get_namespace());
                match message.get_namespace() {
                    namespace::CONNECTION => {
                        serde_json::from_str::<connection::Payload>(message.get_payload_utf8())
                            .map_err(|err| io::Error::new(io::ErrorKind::Other, err))
                            .map(ChannelMessage::Connection)
                            .map(Some)
                    }
                    namespace::HEARTBEAT => {
                        serde_json::from_str::<heartbeat::Payload>(message.get_payload_utf8())
                            .map_err(|err| io::Error::new(io::ErrorKind::Other, err))
                            .map(ChannelMessage::Heartbeat)
                            .map(Some)
                    }
                    namespace::MEDIA => serde_json::from_str::<media::Payload>(message.get_payload_utf8())
                        .map_err(|err| io::Error::new(io::ErrorKind::Other, err))
                        .map(ChannelMessage::Media)
                        .map(Some),
                    namespace::RECEIVER => {
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
        };
        let decode_counter = self.decoded_frames.fetch_add(1usize, Ordering::SeqCst);
        trace!(
            "CastMessageCodec stream=decode frame-counter={} message={:?}",
            decode_counter, message
        );
        message
    }
}

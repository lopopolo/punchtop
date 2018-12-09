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
    decode_counter: AtomicUsize,
    encode_counter: AtomicUsize,
}

impl CastMessageCodec {
    pub fn new() -> Self {
        CastMessageCodec {
            state: DecodeState::Header,
            decode_counter: AtomicUsize::new(0),
            encode_counter: AtomicUsize::new(0),
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
                let message = protobuf::parse_from_bytes::<CastMessage>(&src)
                    .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;
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
        }
    }
}

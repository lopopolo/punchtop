use std::io;

use byteorder::{BigEndian, ByteOrder};
use bytes::{Buf, BufMut, BytesMut, IntoBuf};
use serde_json::from_str;
use tokio::codec::{Decoder, Encoder};

use super::message::namespace;
use super::payload::*;
use super::proto::CastMessage;
use super::provider::*;
use super::{message, ChannelMessage};

/// Protobuf header is a big endian u32.
const CAST_MESSAGE_HEADER_LENGTH: usize = 4;
/// Max message size is [64KB](https://developers.google.com/cast/docs/reference/messages).
const CAST_MESSAGE_PROTOBUF_MAX_LENGTH: usize = 64 << 10;

/// `CastMessageCodec` decodes a length-prefixed protobuf. This enum represents
/// the phase of the decoding. Keep track of the decode phase to ensure the
/// decoder does not drop bytes from the `BytesMut`.
#[derive(Debug)]
enum DecodeState {
    /// Waiting to read a u32 representing the size of the next protobuf.
    Header,
    /// Reading a protobuf with a given length.
    Payload(usize),
}

impl Default for DecodeState {
    fn default() -> Self {
        DecodeState::Header
    }
}

#[derive(Debug, Default)]
pub struct CastMessageCodec {
    state: DecodeState,
    request_id: i64,
    decoded_frames: i64,
    encoded_frames: i64,
}

impl Encoder for CastMessageCodec {
    type Item = Command;
    type Error = io::Error;

    fn encode(&mut self, item: Self::Item, dst: &mut BytesMut) -> Result<(), Self::Error> {
        // A `0` request id is reserved for "spontaneous" messages from the receiver
        // https://developers.google.com/cast/docs/reference/messages#MediaMess
        self.request_id += 1;
        self.encoded_frames += 1;
        trace!(
            "CastMessageCodec stream=encode frame-counter={} command={:?}",
            self.encoded_frames,
            item
        );
        let message = match item {
            Command::Connect(connect) => message::connection::connect(&connect.transport),
            Command::Heartbeat => message::heartbeat::ping(),
            Command::Launch { app_id } => message::receiver::launch(self.request_id, &app_id),
            Command::Load { connect, media } => {
                message::media::load(self.request_id, &connect, *media)
            }
            Command::MediaStatus(connect) => message::media::status(self.request_id, &connect),
            Command::Play(ref connect) => message::media::play(self.request_id, &connect),
            Command::ReceiverStatus => message::receiver::status(self.request_id),
            Command::Stop(ref connect) => message::media::stop(self.request_id, connect),
            _ => unimplemented!(),
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
    /// Cast wire protocol is a 4-byte big endian length-prefixed protobuf. At
    /// least 4 bytes are required to decode the next frame. Read the length of
    /// the following protobuf and reserve that much capacity in the `BytesMut`.
    fn decode_header(&mut self, src: &mut BytesMut) -> Option<usize> {
        if src.len() < CAST_MESSAGE_HEADER_LENGTH {
            return None;
        }
        let header = src.split_to(4);
        let length = {
            let mut header = header.into_buf();
            header.get_u32_be() as usize
        };
        if length > CAST_MESSAGE_PROTOBUF_MAX_LENGTH {
            panic!("CastMessageCodec decoder received message of length {}, which is larger than the max message length of {}", length, CAST_MESSAGE_PROTOBUF_MAX_LENGTH);
        }
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
                Some(n) => n,
                None => return Ok(None),
            },
            DecodeState::Payload(n) => n,
        };
        self.state = DecodeState::Payload(n);
        let message = match self.decode_payload(n, src) {
            Some(mut src) => {
                self.state = DecodeState::Header;
                src.reserve(CAST_MESSAGE_HEADER_LENGTH);
                let message = protobuf::parse_from_bytes::<CastMessage>(&src)
                    .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;
                trace!(
                    "CastMessageCodec stream=decode namespace={}",
                    message.get_namespace()
                );
                match message.get_namespace() {
                    namespace::CONNECTION => {
                        from_str::<connection::Response>(message.get_payload_utf8())
                            .map_err(|err| io::Error::new(io::ErrorKind::Other, err))
                            .map(Box::new)
                            .map(ChannelMessage::Connection)
                            .map(Some)
                    }
                    namespace::HEARTBEAT => {
                        from_str::<heartbeat::Response>(message.get_payload_utf8())
                            .map_err(|err| io::Error::new(io::ErrorKind::Other, err))
                            .map(Box::new)
                            .map(ChannelMessage::Heartbeat)
                            .map(Some)
                    }
                    namespace::MEDIA => from_str::<media::Response>(message.get_payload_utf8())
                        .map_err(|err| io::Error::new(io::ErrorKind::Other, err))
                        .map(Box::new)
                        .map(ChannelMessage::Media)
                        .map(Some),
                    namespace::RECEIVER => {
                        from_str::<receiver::Response>(message.get_payload_utf8())
                            .map_err(|err| io::Error::new(io::ErrorKind::Other, err))
                            .map(Box::new)
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
        self.decoded_frames += 1;
        trace!(
            "CastMessageCodec stream=decode frame-counter={} message={:?}",
            self.decoded_frames,
            message
        );
        message
    }
}

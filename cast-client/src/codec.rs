use std::convert::TryInto;
use std::io;

use byteorder::{BigEndian, ByteOrder};
use bytes::{Buf, BufMut, BytesMut, IntoBuf};
use protobuf::{CodedOutputStream, Message};
use tokio_codec::{Decoder, Encoder};

use crate::channel;
use crate::proto;
use crate::provider::Command;

/// Protobuf header is a big endian u32.
const CAST_MESSAGE_HEADER_LENGTH: usize = 4;
/// Max message size is [64KB](https://developers.google.com/cast/docs/reference/messages).
const CAST_MESSAGE_PROTOBUF_MAX_LENGTH: usize = 64 << 10;

/// `CastMessage` decodes a length-prefixed protobuf. This enum represents
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
pub struct CastMessage {
    state: DecodeState,
    request_id: i64,
    decoded_frames: i64,
    encoded_frames: i64,
}

impl Encoder for CastMessage {
    type Item = Command;
    type Error = io::Error;

    fn encode(&mut self, item: Self::Item, dst: &mut BytesMut) -> Result<(), Self::Error> {
        // A `0` request id is reserved for "spontaneous" messages from the receiver
        // https://developers.google.com/cast/docs/reference/messages#MediaMess
        self.request_id += 1;
        self.encoded_frames += 1;
        trace!(
            "codec encoded frame {} for command {:?}",
            self.encoded_frames,
            item
        );
        let message = match item {
            Command::Connect(connect) => channel::connection::connect(&connect.transport),
            Command::Launch { app_id } => channel::receiver::launch(self.request_id, &app_id),
            Command::Load { connect, media } => {
                channel::media::load(self.request_id, &connect, *media)
            }
            Command::MediaStatus(connect) => channel::media::status(self.request_id, &connect),
            Command::Pause(connect) => channel::media::pause(self.request_id, &connect),
            Command::Ping => channel::heartbeat::ping(),
            Command::Play(connect) => channel::media::play(self.request_id, &connect),
            Command::Pong => channel::heartbeat::pong(),
            Command::ReceiverStatus => channel::receiver::status(self.request_id),
            Command::Stop(connect) => channel::media::stop(self.request_id, &connect),
            _ => unimplemented!(), // TODO: implement all commands
        };

        let mut buf = Vec::new();
        let mut output = CodedOutputStream::new(&mut buf);
        message
            .write_to(&mut output)
            .and_then(|_| output.flush())
            .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;

        if buf.len() > CAST_MESSAGE_PROTOBUF_MAX_LENGTH {
            panic!("CastMessageCodec encoder generated message of length {}, which is larger than the max message length of {}", buf.len(), CAST_MESSAGE_PROTOBUF_MAX_LENGTH);
        }

        // Cast wire protocol is a 4-byte big endian length-prefixed protobuf.
        let header = &mut [0; 4];
        let msg_size = buf
            .len()
            .try_into()
            .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;
        BigEndian::write_u32(header, msg_size);

        dst.reserve(CAST_MESSAGE_HEADER_LENGTH + buf.len());
        dst.put_slice(header);
        dst.put_slice(&buf);
        Ok(())
    }
}

impl CastMessage {
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

    fn try_decode(&mut self, src: &mut BytesMut) -> Result<Option<proto::CastMessage>, io::Error> {
        let n = match self.state {
            DecodeState::Header => match self.decode_header(src) {
                Some(n) => n,
                None => return Ok(None),
            },
            DecodeState::Payload(n) => n,
        };
        self.state = DecodeState::Payload(n);
        if let Some(mut src) = self.decode_payload(n, src) {
            self.state = DecodeState::Header;
            src.reserve(CAST_MESSAGE_HEADER_LENGTH);
            let message = protobuf::parse_from_bytes::<proto::CastMessage>(&src)
                .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;
            self.decoded_frames += 1;
            trace!(
                "codec decoded frame {} for message in namespace {}",
                self.decoded_frames,
                message.get_namespace()
            );
            Ok(Some(message))
        } else {
            Ok(None)
        }
    }
}

impl Decoder for CastMessage {
    type Item = proto::CastMessage;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let item = self.try_decode(src);
        if item.is_err() {
            warn!("Error in decoder: {:?}", item);
        }
        item
    }
}

use protobuf::{CodedOutputStream, ProtobufResult};

pub mod connection;
pub mod heartbeat;
pub mod media;
pub mod receiver;

pub mod namespace {
    pub const CONNECTION: &str = super::connection::NAMESPACE;
    pub const HEARTBEAT: &str = super::heartbeat::NAMESPACE;
    pub const MEDIA: &str = super::media::NAMESPACE;
    pub const RECEIVER: &str = super::receiver::NAMESPACE;
}

pub const DEFAULT_SENDER_ID: &str = "sender-0";
pub const DEFAULT_DESTINATION_ID: &str = "receiver-0";

pub fn encode(msg: &impl protobuf::Message, buf: &mut Vec<u8>) -> ProtobufResult<()> {
    let mut output = CodedOutputStream::new(buf);
    msg.write_to(&mut output)?;
    output.flush()
}

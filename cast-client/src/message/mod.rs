use protobuf::{CodedOutputStream, ProtobufResult};

pub mod connection;
pub mod heartbeat;
pub mod media;
pub mod receiver;

pub const DEFAULT_SENDER_ID: &str = "sender-0";
pub const DEFAULT_DESTINATION_ID: &str = "receiver-0";

pub fn encode(msg: &impl protobuf::Message, buf: &mut Vec<u8>) -> ProtobufResult<()> {
    let mut output = CodedOutputStream::new(buf);
    msg.write_to(&mut output)?;
    output.flush()
}

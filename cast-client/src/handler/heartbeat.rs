use futures::sync::mpsc::UnboundedSender;

use crate::handler::{Error, Handler};
use crate::payload::heartbeat::Response;
use crate::Command;

const NAMESPACE: &str = "urn:x-cast:com.google.cast.tp.heartbeat";
const CHANNEL: &str = "heartbeat";

#[derive(Debug)]
pub struct Heartbeat {
    command: UnboundedSender<Command>,
}

impl Heartbeat {
    pub fn new(command: UnboundedSender<Command>) -> Self {
        Self { command }
    }
}

impl Handler for Heartbeat {
    type Payload = Response;

    fn channel(&self) -> &str {
        CHANNEL
    }

    fn namespace(&self) -> &str {
        NAMESPACE
    }

    fn handle(&self, payload: Self::Payload) -> Result<(), Error> {
        trace!("{} got {:?}", self.channel(), payload);
        match payload {
            Response::Ping => self
                .command
                .unbounded_send(Command::Pong)
                .map_err(|_| Error::CommandSend),
            Response::Pong => Ok(()),
        }
    }
}

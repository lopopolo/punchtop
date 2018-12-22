use crate::handler::{Error, Handler};
use crate::payload::connection::Response;

const NAMESPACE: &str = "urn:x-cast:com.google.cast.tp.connection";
const CHANNEL: &str = "connection";

#[derive(Debug)]
pub struct Connection;

impl Handler for Connection {
    type Payload = Response;

    fn channel(&self) -> &str {
        CHANNEL
    }

    fn namespace(&self) -> &str {
        NAMESPACE
    }

    fn handle(&self, _: Self::Payload) -> Result<(), Error> {
        warn!("cast connection closed");
        Ok(())
    }
}

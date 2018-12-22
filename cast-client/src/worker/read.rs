use std::error;
use std::fmt;
use std::io;

use futures::prelude::*;
use futures::sync::mpsc::UnboundedSender;
use futures::Future;
use futures_locks::RwLock;

use crate::payload::*;
use crate::session;
use crate::{ChannelMessage, Command, ConnectState, Status, DEFAULT_MEDIA_RECEIVER_APP_ID};

#[derive(Debug)]
enum ChannelError {
    CommandSend(String),
    StatusSend(String),
    UnknownPayload(String),
}

impl error::Error for ChannelError {}

impl fmt::Display for ChannelError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ChannelError::CommandSend(ref channel) => {
                write!(f, "Unable to send command from {} channel", channel)
            }
            ChannelError::StatusSend(ref channel) => {
                write!(f, "Unable to send status from {} channel", channel)
            }
            ChannelError::UnknownPayload(ref channel) => {
                write!(f, "Received unknown payload on {} channel", channel)
            }
        }
    }
}

pub(crate) fn task(
    source: impl Stream<Item = ChannelMessage, Error = io::Error>,
    connect_state: RwLock<ConnectState>,
    status: UnboundedSender<Status>,
    command: UnboundedSender<Command>,
) -> impl Future<Item = (), Error = ()> {
    source
        .for_each(move |message| read(message, &connect_state, status.clone(), command.clone()))
        .map_err(|err| warn!("Error on read: {:?}", err))
}

fn read(
    message: ChannelMessage,
    connect: &RwLock<ConnectState>,
    tx: UnboundedSender<Status>,
    command: UnboundedSender<Command>,
) -> Result<(), io::Error> {
    let read = match message {
        ChannelMessage::Heartbeat(message) => do_heartbeat(&*message, &command),
        ChannelMessage::Media(message) => do_media(*message, &tx, connect),
        ChannelMessage::Receiver(message) => do_receiver(*message, tx, command, connect),
        _ => Err(ChannelError::UnknownPayload("unknown".to_owned())),
    };
    read.map_err(|err| io::Error::new(io::ErrorKind::Other, err))
}

fn do_heartbeat(
    message: &heartbeat::Response,
    command: &UnboundedSender<Command>,
) -> Result<(), ChannelError> {
    use crate::payload::heartbeat::Response::*;
    match message {
        Ping => {
            trace!("heartbeat got PING");
            command
                .unbounded_send(Command::Pong)
                .map_err(|_| ChannelError::CommandSend("heartbeat".to_owned()))
        }
        Pong => {
            trace!("heartbeat got PONG");
            Ok(())
        }
    }
}

fn do_media(
    message: media::Response,
    tx: &UnboundedSender<Status>,
    connect: &RwLock<ConnectState>,
) -> Result<(), ChannelError> {
    use crate::payload::media::Response::*;
    match message {
        MediaStatus { status, .. } => {
            let status = status.into_iter().next();
            let media_session = status.as_ref().map(|status| status.media_session_id);
            match media_session {
                Some(media_session) => {
                    let tx = tx.clone();
                    let task = session::register(connect, media_session);
                    let task = task.and_then(move |connect| {
                        if let Some(connect) = connect {
                            tx.unbounded_send(Status::MediaConnected(Box::new(connect)))
                                .map(|_| ())
                                .map_err(|_| {
                                    warn!("{}", ChannelError::StatusSend("media".to_owned()))
                                })
                        } else {
                            Ok(())
                        }
                    });
                    tokio_executor::spawn(task)
                }
                None => tokio_executor::spawn(session::invalidate(connect)),
            };
            if let Some(state) = status {
                tx.unbounded_send(Status::MediaState(Box::new(state)))
                    .map_err(|_| ChannelError::StatusSend("media".to_owned()))?;
            }
            Ok(())
        }
        _ => Err(ChannelError::UnknownPayload("media".to_owned())),
    }
}

fn do_receiver(
    message: receiver::Response,
    tx: UnboundedSender<Status>,
    command: UnboundedSender<Command>,
    connect: &RwLock<ConnectState>,
) -> Result<(), ChannelError> {
    use crate::payload::receiver::Response::*;
    let ReceiverStatus { status, .. } = message;
    let app = status
        .applications
        .iter()
        .find(|app| app.app_id == DEFAULT_MEDIA_RECEIVER_APP_ID);
    let session = app.map(|app| app.session_id.to_owned());
    let transport = app.map(|app| app.transport_id.to_owned());
    let connect = connect.write().and_then(move |mut state| {
        trace!("Acquired connect state lock in receiver status");
        if !state.set_session(session.deref()) || !state.set_transport(transport.deref()) {
            // Connection did not change
            return Ok(());
        }
        if let Some(ref connect) = state.receiver_connection() {
            debug!("Connecting to transport {}", connect.transport);
            tx.unbounded_send(Status::Connected(Box::new(connect.clone())))
                .map_err(|_| ())?;
            // we've connected to the default receiver. Now connect to the
            // transport backing the launched app session.
            command
                .unbounded_send(Command::Connect(connect.clone()))
                .map_err(|_| ())?;
        }
        Ok(())
    });
    tokio_executor::spawn(connect);
    Ok(())
}

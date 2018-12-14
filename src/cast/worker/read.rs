use std::io;

use futures::prelude::*;
use futures::sync::mpsc::UnboundedSender;
use futures::Future;
use futures_locks::Mutex;

use cast::payload::*;
use cast::worker::status::{invalidate_media_connection, register_media_session};
use cast::{ChannelMessage, Command, ConnectState, Status, DEFAULT_MEDIA_RECEIVER_APP_ID};

pub fn task(
    source: impl Stream<Item = ChannelMessage, Error = io::Error>,
    connect_state: Mutex<ConnectState>,
    status: UnboundedSender<Status>,
    command: UnboundedSender<Command>,
) -> impl Future<Item = (), Error = ()> {
    source
        .for_each(move |message| {
            read(
                message,
                connect_state.clone(),
                status.clone(),
                command.clone(),
            );
            Ok(())
        })
        .map(|_| ())
        .map_err(|err| warn!("Error on send: {:?}", err))
}

fn read(
    message: ChannelMessage,
    connect: Mutex<ConnectState>,
    tx: UnboundedSender<Status>,
    command: UnboundedSender<Command>,
) {
    match message {
        ChannelMessage::Heartbeat(_) => trace!("Got heartbeat"),
        ChannelMessage::Media(message) => do_media(*message, tx, connect),
        ChannelMessage::Receiver(message) => do_receiver(*message, tx, command, connect),
        payload => warn!("Got unknown payload: {:?}", payload),
    }
}

fn do_media(message: media::Response, tx: UnboundedSender<Status>, connect: Mutex<ConnectState>) {
    use cast::payload::media::Response::*;
    match message {
        MediaStatus { status, .. } => {
            let status = status.into_iter().next();
            let media_session = status.as_ref().map(|status| status.media_session_id);
            match media_session {
                Some(media_session) => {
                    let tx = tx.clone();
                    let task = register_media_session(connect, media_session);
                    let task = task.map(move |connect| {
                        if let Some(connect) = connect {
                            let _ = tx.unbounded_send(Status::MediaConnected(Box::new(connect)));
                        }
                    });
                    tokio::spawn(task)
                }
                None => tokio::spawn(invalidate_media_connection(connect)),
            };
            if let Some(status) = status {
                let _ = tx.unbounded_send(Status::MediaStatus(Box::new(status)));
            }
        }
        payload => warn!("Got unknown payload on media channel: {:?}", payload),
    }
}

fn do_receiver(
    message: receiver::Response,
    tx: UnboundedSender<Status>,
    command: UnboundedSender<Command>,
    connect: Mutex<ConnectState>,
) {
    use cast::payload::receiver::Response::*;
    let ReceiverStatus { status, .. } = message;
    let app = status
        .applications
        .iter()
        .find(|app| app.app_id == DEFAULT_MEDIA_RECEIVER_APP_ID);
    let session = app.map(|app| app.session_id.to_owned());
    let transport = app.map(|app| app.transport_id.to_owned());
    let connect = connect.lock().map(move |mut state| {
        trace!("Acquired connect state lock in receiver status");
        let did_connect =
            state.set_session(session.deref()) && state.set_transport(transport.deref());
        if let (Some(ref connect), true) = (state.receiver_connection(), did_connect) {
            debug!("Connecting to transport {}", connect.transport);
            let _ = tx.unbounded_send(Status::Connected(Box::new(connect.clone())));
            // we've connected to the default receiver. Now connect to
            // the transport backing the launched app session.
            let _ = command.unbounded_send(Command::Connect(connect.clone()));
        }
    });
    tokio::spawn(connect);
}

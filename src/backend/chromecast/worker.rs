///! Implements Chromecast player on a separate thread.
use std::net::SocketAddr;
use std::thread;

use crossbeam_channel::{Receiver, Sender};
use rust_cast::channels::media::{Media, StatusEntry};
use rust_cast::channels::receiver::{Application, CastDeviceApp};
use rust_cast::CastDevice;

use backend::Error;

pub type CastResult = Result<Status, Error>;

pub struct Channel {
    tx: Sender<CastResult>,
    rx: Receiver<Control>,
}

pub enum Control {
    Close,
    Load(Box<Media>),
    Stop,
}

pub enum Status {
    Closed,
    Connected,
    Loaded,
    Stopped,
}

pub fn chan(tx: Sender<CastResult>, rx: Receiver<Control>) -> Channel {
    Channel { tx, rx }
}

pub fn spawn(addr: SocketAddr, chan: Channel) {
    thread::spawn(move || runloop(addr, chan));
    ()
}

fn runloop(addr: SocketAddr, chan: Channel) {
    let (device, app) = match connect(addr) {
        Ok(connection) => {
            let _ = chan.tx.try_send(Ok(Status::Connected));
            connection
        }
        Err(err) => {
            let _ = chan.tx.send(Err(err));
            return;
        }
    };
    loop {
        select! {
            recv(chan.rx) -> msg => match msg {
                Ok(Control::Close) => {
                    let close = device
                        .receiver
                        .stop_app(&app.session_id[..])
                        .map_err(Error::Cast)
                        .map(|_| Status::Closed);
                    let _ = chan.tx.try_send(close);
                },
                Ok(Control::Load(media)) => {
                    let load = device
                        .media
                        .load(&app.transport_id[..], &app.session_id[..], &media)
                        .map_err(Error::Cast)
                        .map(|_| Status::Loaded);
                    let _ = chan.tx.try_send(load);
                },
                Ok(Control::Stop) => {
                    match status(&device, &app) {
                        Ok(entries) => {
                            let mut succeed = true;
                            for entry in entries {
                                let stop = device
                                    .media
                                    .stop(&app.transport_id[..], entry.media_session_id)
                                    .map_err(Error::Cast);
                                if let Err(stop) = stop {
                                    let _ = chan.tx.try_send(Err(stop));
                                    succeed = false;
                                }
                            }
                            if succeed {
                                let _ = chan.tx.try_send(Ok(Status::Stopped));
                            }
                        }
                        Err(err) => {
                            let _ = chan.tx.try_send(Err(err));
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

fn connect<'a>(addr: SocketAddr) -> Result<(CastDevice<'a>, Application), Error> {
    let ip = format!("{}", addr.ip());
    match CastDevice::connect_without_host_verification(ip, addr.port()) {
        Err(_) => Err(Error::BackendNotInitialized),
        Ok(device) => {
            // TODO: use a custom styled media receiver - https://developers.google.com/cast/v2/receiver_apps#Styled
            let sink = CastDeviceApp::DefaultMediaReceiver;
            let app = device
                .connection
                .connect("receiver-0")
                .and_then(|_| device.receiver.launch_app(&sink))
                .and_then(|app| {
                    device
                        .connection
                        .connect(&app.transport_id[..])
                        .map(|_| app)
                });
            app.map_err(Error::Cast).map(|app| (device, app))
        }
    }
}

fn status(device: &CastDevice, app: &Application) -> Result<Vec<StatusEntry>, Error> {
    device
        .media
        .get_status(&app.transport_id[..], None)
        .map_err(Error::Cast)
        .map(|status| status.entries)
}

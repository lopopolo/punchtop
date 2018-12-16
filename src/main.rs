#![feature(inner_deref)]
#![feature(proc_macro_hygiene, decl_macro)]

extern crate base64;
extern crate byteorder;
extern crate bytes;
extern crate env_logger;
extern crate floating_duration;
extern crate futures;
extern crate futures_locks;
extern crate hostname;
#[macro_use]
extern crate log;
extern crate mdns;
extern crate mp3_duration;
extern crate mp4parse as mp4;
extern crate native_tls;
extern crate neguse_taglib;
extern crate neguse_types;
#[macro_use]
extern crate nom;
extern crate openssl;
extern crate protobuf;
extern crate rand;
#[macro_use]
extern crate rocket;
extern crate rodio;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate tokio;
extern crate tokio_tls;
extern crate tree_magic;
extern crate url;
extern crate walkdir;
extern crate web_view;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures::prelude::*;
use futures::Stream;
use serde_json::to_string;
use tokio::runtime::Runtime;
use web_view::*;

mod app;
mod backend;
mod cast;
mod playlist;
mod stream;

use app::{AppConfig, AppController, AppEvent};
use backend::chromecast::Device;
use stream::drain;

const CAST: &str = "Kitchen Home";

fn main() {
    env_logger::init();
    let mut rt = Runtime::new().unwrap();
    let root = PathBuf::from("/Users/lopopolo/Downloads/test");
    let config = AppConfig {
        duration: Duration::new(60, 0),
        iterations: 10,
    };
    let player = backend::chromecast::devices().find(|p| p.name == CAST);
    let player = match player {
        Some(player) => player,
        None => {
            eprintln!("Could not find chromecast named {}", CAST);
            ::std::process::exit(1);
        }
    };
    let playlist = playlist::Playlist::from_directory(&root, &config);
    let (client, chan) = match Device::connect(player, playlist.registry(), &mut rt) {
        Ok(connect) => connect,
        Err(err) => {
            warn!("chromecast connect error: {:?}", err);
            eprintln!("Could not connect to chromecast named {}", CAST);
            ::std::process::exit(1);
        }
    };
    let (controller, shutdown) = AppController::new(config, playlist, client);
    let controller = Arc::new(Mutex::new(controller));
    let handler_controller = Arc::clone(&controller);
    let io_controller = Arc::clone(&controller);
    let mut webview = web_view::builder()
        .title("Punchtop")
        .content(Content::Url("http://localhost:8080/"))
        .size(480, 720)
        .resizable(false)
        .debug(true)
        .user_data(())
        .invoke_handler(move |_webview, arg| {
            let controller = handler_controller.lock().map_err(|_| Error::Dispatch)?;
            warn!("webview invoke arg: {}", arg);
            match arg {
                "play" => controller.play(),
                "pause" => controller.pause(),
                _ => unimplemented!(),
            };
            Ok(())
        })
        .build()
        .unwrap();
    let _ = webview.set_color((15, 55, 55));
    let ui_handle = webview.handle();
    let play_loop = drain(chan, shutdown.map_err(|_| ()))
        .for_each(move |event| {
            let mut controller = io_controller.lock().map_err(|_| ())?;
            match controller.handle(event) {
                Some(AppEvent::Shutdown) => {
                    if let Ok(json) = to_string(&AppEvent::ClearMedia) {
                        let _ = ui_handle.dispatch(move |webview| {
                            let eval = format!("store.dispatch({})", json);
                            let _ = webview.eval(&eval);
                            webview.terminate();
                            Ok(())
                        });
                    }
                }
                Some(ref event @ AppEvent::SetMedia { .. }) => {
                    if let Ok(json) = to_string(event) {
                        let _ = ui_handle.dispatch(move |webview| {
                            let eval = format!("store.dispatch({})", json);
                            webview.eval(&eval)
                        });
                    }
                    if let Ok(json) = to_string(&AppEvent::SetPlayback { is_playing: true }) {
                        let _ = ui_handle.dispatch(move |webview| {
                            let eval = format!("store.dispatch({})", json);
                            webview.eval(&eval)
                        });
                    }
                }
                Some(ref event @ AppEvent::SetElapsed { .. }) => {
                    if let Ok(json) = to_string(event) {
                        let _ = ui_handle.dispatch(move |webview| {
                            let eval = format!("store.dispatch({})", json);
                            webview.eval(&eval)
                        });
                    }
                }
                Some(event) => warn!("Unknown app ui event: {:?}", event),
                None => (),
            };
            Ok(())
        })
        .into_future();
    rt.spawn(play_loop);
    webview.run().unwrap();
    rt.shutdown_on_idle().wait().unwrap();
}

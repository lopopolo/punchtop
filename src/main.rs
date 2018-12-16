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

use floating_duration::TimeAsFloat;
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
    let root = PathBuf::from("/Users/lopopolo/Downloads/keys");
    let config = AppConfig {
        duration: Duration::new(20, 0),
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
        .invoke_handler(move |webview, arg| {
            let mut controller = handler_controller.lock().map_err(|_| Error::Dispatch)?;
            warn!("webview invoke arg: {}", arg);
            match arg {
                "init" => {
                    dispatch_in_webview(
                        webview,
                        &AppEvent::SetConfig {
                            duration: controller.config.duration.as_fractional_secs(),
                        },
                    );
                    controller.signal_view_initialized();
                }
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
            let mut shutdown = false;
            for event in controller.handle(event) {
                match event {
                    AppEvent::Shutdown => shutdown = true,
                    _ => {}
                };
                let _ = ui_handle.dispatch(move |webview| {
                    dispatch_in_webview(webview, &event);
                    Ok(())
                });
            }
            if shutdown {
                let _ = ui_handle.dispatch(|webview| {
                    webview.terminate();
                    Ok(())
                });
            }
            Ok(())
        })
        .into_future();
    rt.spawn(play_loop);
    webview.run().unwrap();
    rt.shutdown_on_idle().wait().unwrap();
}

fn dispatch_in_webview(webview: &mut WebView<()>, event: &AppEvent) {
    let eval = to_string(event).map(|json| {
        let eval = format!("store.dispatch({})", json);
        webview.eval(&eval)
    });
    if let Err(err) = eval {
        warn!("err in webview eval: {:?}", err);
    }
}

#![feature(duration_as_u128, inner_deref, try_from)]
#![feature(proc_macro_hygiene, decl_macro)]
#![warn(clippy::all, clippy::pedantic)]

#[macro_use]
extern crate log;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use floating_duration::TimeAsFloat;
use futures::prelude::*;
use futures::Stream;
use punchtop_audio::chromecast::{devices, Device};
use punchtop_playlist::fs;
use serde_json::to_string;
use stream_util::Drainable;
use tokio::runtime::Runtime;
use web_view::*;

mod app;

use crate::app::{Config, Controller, Event, Lifecycle};

const CAST: &str = "Kitchen Home";

fn main() {
    env_logger::init();
    let mut rt = Runtime::new().unwrap();
    let config = Config {
        duration: Duration::new(60, 0),
        iterations: 5,
    };
    let player = devices().find(|p| p.name == CAST);
    let player = if let Some(player) = player {
        player
    } else {
        eprintln!("Could not find chromecast named {}", CAST);
        ::std::process::exit(1);
    };
    let playlist = fs::music::new(config.duration, config.iterations).unwrap();
    let (client, chan, connect) = match Device::connect(&player, playlist.registry()) {
        Ok(connect) => connect,
        Err(err) => {
            warn!("chromecast connect error: {:?}", err);
            eprintln!("Could not connect to chromecast named {}", CAST);
            ::std::process::exit(1);
        }
    };
    rt.spawn(connect);
    let (mut controller, valve) = Controller::new(config, playlist);
    controller.set_client(client);
    let controller = Arc::new(Mutex::new(controller));
    let handler_controller = Arc::clone(&controller);
    let io_controller = Arc::clone(&controller);
    let mut webview = web_view::builder()
        .title("Punchtop")
        .content(Content::Html(include_str!(
            "../../punchtop-ui-react/dist/index.html"
        )))
        .size(480, 720)
        .resizable(false)
        .debug(true)
        .user_data(())
        .invoke_handler(move |webview, arg| {
            let mut controller = handler_controller.lock().map_err(|_| Error::Dispatch)?;
            info!("webview invoke handler {}", arg);
            match arg {
                "init" => {
                    dispatch_in_webview(
                        webview,
                        &Event::SetConfig {
                            duration: controller.config.duration.as_fractional_secs(),
                        },
                    );
                    dispatch_in_webview(
                        webview,
                        &Event::SetPlaylist {
                            name: controller.playlist_name().to_owned(),
                        },
                    );
                    controller.view_did_load();
                }
                "play" => controller.play(),
                "pause" => controller.pause(),
                _ => unimplemented!(),
            };
            Ok(())
        })
        .build()
        .unwrap();
    webview.set_color((15, 55, 55));
    let ui_handle = webview.handle();
    let play_loop = chan.drain(valve).for_each(move |event| {
        let mut controller = io_controller.lock().map_err(|_| ())?;
        for event in controller.handle(event) {
            let _ = ui_handle.dispatch(move |webview| {
                dispatch_in_webview(webview, &event);
                Ok(())
            });
        }
        Ok(())
    });
    rt.spawn(play_loop);
    loop {
        match webview.step() {
            Some(Ok(_)) => (),
            Some(Err(e)) => warn!("Error in webview runloop: {:?}", e),
            None => break,
        }
        let shutdown = controller.lock().ok().map_or(false, |controller| {
            controller.view_lifecycle() == &Lifecycle::Terminating
        });
        if shutdown {
            debug!("Shutting down webview runloop");
            break;
        }
    }
    debug!("webview runloop completed");
    webview.terminate();
    debug!("webview terminated");
    rt.shutdown_on_idle().wait().unwrap();
    debug!("tokio runloop completed");
}

fn dispatch_in_webview(webview: &mut WebView<()>, event: &Event) {
    let eval = to_string(event).map(|json| {
        let eval = format!("store.dispatch({})", json);
        webview.eval(&eval)
    });
    if let Err(err) = eval {
        warn!("err in webview eval: {:?}", err);
    }
}

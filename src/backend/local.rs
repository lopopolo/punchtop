use std::ffi::CStr;
use std::fs::File;
use std::io::BufReader;
use std::os::raw::c_char;
use std::path::Path;
use std::time::Duration;

use hostname::get_hostname;
use objc::runtime::Object;
use rodio::{self, Decoder, Sink, Source};

use backend::{Error, Player, PlayerKind};

/// Return a readable computer name using the localized name given
/// by `NSHost` on macOS.
#[cfg(target_os = "macos")]
fn computer_name() -> Option<String> {
    let host = class!(NSHost);
    unsafe {
        let host: *mut Object = msg_send![host, currentHost];
        let name: *mut Object = msg_send![host, localizedName];
        let cstr: *const c_char = msg_send![name, UTF8String];
        CStr::from_ptr(cstr).to_str().ok().map(String::from)
    }
}

/// Return a readable computer name using the localized name given
/// by `UIDevice` on iOS.
#[cfg(target_os = "ios")]
fn computer_name() -> Option<String> {
    let device = class!(UIDevice);
    unsafe {
        let device: *mut Object = msg_send![device, currentDevice];
        let name: *mut Object = msg_send![device, name];
        let cstr: *const c_char = msg_send![name, UTF8String];
        CStr::from_ptr(cstr).to_str().ok().map(String::from)
    }
}

/// Fallback to `get_hostname`.
#[cfg(not(any(target_os = "macos", target_os = "ios")))]
fn computer_name() -> Option<String> {
    None
}

pub struct Device {
    sink: Sink,
}

impl Device {
    pub fn new<'a>() -> Result<Self, Error<'a>> {
        rodio::default_output_device()
            .map(|device| Device {
                sink: Sink::new(&device),
            })
            .ok_or(Error::BackendNotInitialized)
    }
}

impl Player for Device {
    fn name(&self) -> String {
        computer_name()
            .or_else(get_hostname)
            .unwrap_or_else(|| "Local".to_owned())
    }

    fn kind(&self) -> PlayerKind {
        PlayerKind::Local
    }

    fn connect<'a>(&mut self, _root: &'a Path) -> Result<(), Error<'a>> {
        Ok(())
    }

    fn close<'a>(&self) -> Result<(), Error<'a>> {
        Ok(())
    }

    fn play<'a>(&self, path: &'a Path, duration: Duration) -> Result<(), Error<'a>> {
        File::open(path)
            .map_err(|_| Error::CannotLoadMedia(path))
            .and_then(|f| Decoder::new(BufReader::new(f)).map_err(Error::Rodio))
            .map(|source| source.buffered())
            .map(|source| source.take_duration(duration))
            .map(|source| {
                self.sink.append(source);
                self.sink.sleep_until_end();
            })
    }
}

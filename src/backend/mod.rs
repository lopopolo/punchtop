pub mod chromecast;
pub mod local;

use std::path::Path;
use std::time::Duration;

#[derive(Debug)]
pub enum Error<'a> {
    CannotLoadMedia(&'a Path),
    PlaybackFailed,
    BackendNotInitialized,
}

/// Represents an audio device
pub enum Device<'a> {
    /// Local playback using a `rodio` backend.
    Local(local::Device),
    /// Chromecast playback.
    Chromecast(chromecast::Device<'a>),
}

impl<'p> Player for Device<'p> {
    fn name(&self) -> String {
        match self {
            Device::Local(device) => device.name(),
            Device::Chromecast(device) => device.name(),
        }
    }

    fn connect<'a>(&mut self) -> Result<(), Error<'a>> {
        match self {
            Device::Local(device) => device.connect(),
            Device::Chromecast(device) => device.connect(),
        }
    }

    fn close<'a>(&self) -> Result<(), Error<'a>> {
        match self {
            Device::Local(device) => device.close(),
            Device::Chromecast(device) => device.close(),
        }
    }

    fn play<'a, T: AsRef<Path>>(&self, path: &'a T, duration: Duration) -> Result<(), Error<'a>> {
        match self {
            Device::Local(device) => device.play(path, duration),
            Device::Chromecast(device) => device.play(path, duration),
        }
    }
}

/// An iterator yielding `Device`s available for audio playback.
///
/// See [`devices()`](fn.devices.html).
pub struct Devices<'a>(std::vec::IntoIter<Device<'a>>);

impl<'a> Iterator for Devices<'a> {
    type Item = Device<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

/// An iterator yielding `Device`s available for audio playback.
pub fn devices<'a>() -> Devices<'a> {
    let mut devices = vec![];
    if let Ok(local) = local::Device::new() {
        println!("local");
        devices.push(Device::Local(local));
    }
    for chromecast in chromecast::devices() {
        println!("{}", chromecast.name());
        devices.push(Device::Chromecast(chromecast));
    }
    Devices(devices.into_iter())
}

/// Represents an audio player that can enqueue tracks for playback.
///
/// Players must support playing tracks for a fixed duration. Players can assume that
/// all tracks passed to them are at least as long as the supplied duration.
pub trait Player {
    /// Display name for the Player.
    fn name(&self) -> String;

    /// Initialize the player to make it active.
    fn connect<'a>(&mut self) -> Result<(), Error<'a>>;

    /// Close a player to make it inactive.
    fn close<'a>(&self) -> Result<(), Error<'a>>;

    /// Play the media located at `path` for `duration`. Block until `duration` has
    /// elapsed and then stop playing the media.
    fn play<'a, T: AsRef<Path>>(&self, path: &'a T, duration: Duration) -> Result<(), Error<'a>>;
}

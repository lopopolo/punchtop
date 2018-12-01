pub mod chromecast;
pub mod local;

use std::path::Path;
use std::time::Duration;

pub enum Error<'a> {
    CannotLoadMedia(&'a Path),
    PlaybackFailed,
    BackendNotInitialized,
}

/// Represents an audio device
pub enum Device {
    /// Local playback using a `rodio` backend.
    Local(local::Device),
    /// Chromecast playback.
    Chromecast(chromecast::Device),
}

impl Player for Device {
    fn name(&self) -> String {
        match self {
            Device::Local(device) => device.name(),
            Device::Chromecast(device) => device.name(),
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
pub struct Devices(std::vec::IntoIter<Device>);

impl Iterator for Devices {
    type Item = Device;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

/// An iterator yielding `Device`s available for audio playback.
pub fn devices() -> Devices {
    let mut devices = vec![];
    if let Ok(local) = local::Device::new() {
        devices.push(Device::Local(local));
    }
    for chromecast in chromecast::Discovery::poll() {
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

    /// Play the media located at `path` for `duration`. Block until `duration` has
    /// elapsed and then stop playing the media.
    fn play<'a, T: AsRef<Path>>(&self, path: &'a T, duration: Duration) -> Result<(), Error<'a>>;
}

use std::path::{Path, PathBuf};
use std::time::Duration;

use rodio;
use rust_cast;

pub mod chromecast;
pub mod local;
pub mod media_server;

/// Result type for player operations.
pub type Result = std::result::Result<(), Error>;

/// Error wrapper for all player backends.
#[derive(Debug)]
pub enum Error {
    BackendNotInitialized,
    CannotLoadMedia(PathBuf),
    Cast(rust_cast::errors::Error),
    Internal(String),
    Rodio(rodio::decoder::DecoderError),
}

/// Represents an player backend kind.
#[derive(Debug, PartialEq)]
pub enum PlayerKind {
    /// Local playback using a `rodio` backend.
    Local,
    /// Chromecast playback.
    Chromecast,
}

/// An iterator yielding `Players`s available for audio playback.
///
/// See [`players()`](fn.devices.html).
pub struct Players(std::vec::IntoIter<Box<Player>>);

impl Iterator for Players {
    type Item = Box<Player>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

/// An iterator yielding `Device`s available for audio playback.
pub fn players() -> Players {
    let mut devices: Vec<Box<Player>> = vec![];
    if let Ok(local) = local::Device::new() {
        println!("Found local device: {}", local.name());
        devices.push(Box::new(local));
    }
    for chromecast in chromecast::devices() {
        println!("Found chromecast device: {}", chromecast.name());
        devices.push(Box::new(chromecast));
    }
    Players(devices.into_iter())
}

/// Represents an audio player that can enqueue tracks for playback.
///
/// Players must support playing tracks for a fixed duration. Players can assume that
/// all tracks passed to them are at least as long as the supplied duration.
pub trait Player {
    /// Display name for the Player.
    fn name(&self) -> String;

    /// The type of player backend.
    fn kind(&self) -> PlayerKind;

    /// Initialize the player to make it active.
    fn connect(&mut self, root: &Path) -> Result;

    /// Close a player to make it inactive.
    fn close(&self) -> Result;

    /// Play the media located at `path` for `duration`. Block until `duration` has
    /// elapsed and then stop playing the media.
    fn play(&self, path: &Path, duration: Duration) -> Result;
}

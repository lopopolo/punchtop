use rodio;

use playlist::Track;

pub mod chromecast;
pub mod local;

/// Result type for player operations.
pub type Result = std::result::Result<(), Error>;

/// Error wrapper for all player backends.
#[derive(Debug)]
pub enum Error {
    BackendNotInitialized,
    CannotLoadMedia(Track),
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

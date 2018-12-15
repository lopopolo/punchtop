pub mod chromecast;

/// Result type for player operations.
pub type Result = std::result::Result<(), Error>;

/// Error wrapper for all player backends.
#[derive(Debug)]
pub enum Error {
    BackendNotInitialized,
    CannotLoadMedia,
}

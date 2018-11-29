pub mod chromecast;
pub mod local;

use std::path::Path;
use std::time::Duration;

pub enum Error<'a> {
    CannotLoadMedia(&'a Path),
    PlaybackFailed,
    BackendNotInitialized,
}

pub trait BackendDevice {
    /// Play the media located at `path` for `duration`. Block until `duration` has
    /// elapsed and stop playing the media.
    fn play<'a>(&self, path: &'a Path, duration: Duration) -> Result<(), Error<'a>>;
}

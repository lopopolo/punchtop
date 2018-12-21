use std::time::Duration;

use directories::UserDirs;

use crate::fs::{playlist, Playlist};

pub fn new(duration: Duration, iterations: u64) -> Option<Playlist> {
    let user_dirs = UserDirs::new()?;
    let root = user_dirs.audio_dir()?;
    let (elapsed, playlist) =
        elapsed::measure_time(|| playlist(&root, "My Music", duration, iterations));
    info!(
        "Took {} building a {} item playlist from {:?}",
        elapsed, iterations, root
    );
    Some(playlist)
}

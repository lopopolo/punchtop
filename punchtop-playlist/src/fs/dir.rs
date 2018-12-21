use std::path::Path;
use std::time::Duration;

use crate::fs::{playlist, Playlist};

pub fn new(root: &Path, duration: Duration, iterations: u64) -> Option<Playlist> {
    let name = root.file_name().map_or_else(
        || "Playlist".to_owned(),
        |name| name.to_string_lossy().into_owned(),
    );
    let (elapsed, playlist) =
        elapsed::measure_time(|| playlist(&root, &name, duration, iterations));
    info!(
        "Took {} building a {} item playlist from {:?}",
        elapsed, iterations, root
    );
    Some(playlist)
}

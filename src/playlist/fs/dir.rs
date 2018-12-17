use std::path::Path;

use app::Config;

pub fn new(root: &Path, config: &Config) -> Option<super::Playlist> {
    let name = root.file_name().map_or_else(
        || "Playlist".to_owned(),
        |name| name.to_string_lossy().into_owned(),
    );
    Some(super::new(root, &name, config))
}

use std::path::Path;

use app::AppConfig;

pub fn new(root: &Path, config: &AppConfig) -> Option<super::Playlist> {
    let name = root
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Playlist".to_owned());
    Some(super::new(root, &name, config))
}

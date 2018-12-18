use std::path::Path;

use app::Config;

pub fn new(root: &Path, config: &Config) -> Option<super::Playlist> {
    let name = root.file_name().map_or_else(
        || "Playlist".to_owned(),
        |name| name.to_string_lossy().into_owned(),
    );
    let (elapsed, playlist) = elapsed::measure_time(|| {
        super::new(&root, &name, config)
    });
    info!("Took {} building a {} item playlist from {:?}", elapsed, config.iterations, root);
    Some(playlist)
}

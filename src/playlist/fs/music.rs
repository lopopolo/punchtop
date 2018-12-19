use dirs::audio_dir;

use crate::app::Config;

pub fn new(config: &Config) -> Option<super::Playlist> {
    audio_dir().map(|root| {
        let (elapsed, playlist) = elapsed::measure_time(|| super::new(&root, "My Music", config));
        info!(
            "Took {} building a {} item playlist from {:?}",
            elapsed, config.iterations, root
        );
        playlist
    })
}

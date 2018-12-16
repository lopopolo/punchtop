use dirs::audio_dir;

use app::AppConfig;

pub fn new(config: &AppConfig) -> Option<super::Playlist> {
    audio_dir().map(|root| super::new(&root, "My Music", config))
}

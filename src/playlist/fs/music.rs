use dirs::audio_dir;

use app::Config;

pub fn new(config: &Config) -> Option<super::Playlist> {
    audio_dir().map(|root| super::new(&root, "My Music", config))
}

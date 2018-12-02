use backend::{Error, Player, PlayerKind};
use hostname::get_hostname;
use rodio::{self, Decoder, Sink, Source};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::time::Duration;

pub struct Device {
    sink: Sink,
}

impl Device {
    pub fn new<'a>() -> Result<Self, Error<'a>> {
        rodio::default_output_device()
            .map(|device| Device {
                sink: Sink::new(&device),
            })
            .ok_or(Error::BackendNotInitialized)
    }
}

impl Player for Device {
    fn name(&self) -> String {
        get_hostname().unwrap_or_else(|| "Local".to_owned())
    }

    fn kind(&self) -> PlayerKind {
        PlayerKind::Local
    }

    fn connect<'a>(&mut self, _root: &'a Path) -> Result<(), Error<'a>> {
        Ok(())
    }

    fn close<'a>(&self) -> Result<(), Error<'a>> {
        Ok(())
    }

    fn play<'a>(&self, path: &'a Path, duration: Duration) -> Result<(), Error<'a>> {
        File::open(path)
            .map_err(|_| Error::CannotLoadMedia(path))
            .and_then(|f| Decoder::new(BufReader::new(f)).map_err(|_| Error::PlaybackFailed))
            .map(|source| source.buffered())
            .map(|source| source.take_duration(duration))
            .map(|source| {
                self.sink.append(source);
                self.sink.sleep_until_end();
            })
    }
}

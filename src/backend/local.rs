use backend;
use backend::Error;
use rodio::{self, Decoder, Sink, Source};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::time::Duration;

pub struct BackendDevice {
    sink: Sink,
}

impl BackendDevice {
    pub fn new<'a>() -> Result<Self, Error<'a>> {
        rodio::default_output_device()
            .map(|device| BackendDevice { sink: Sink::new(&device) })
            .ok_or(Error::BackendNotInitialized)
    }
}

impl backend::BackendDevice for BackendDevice {
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

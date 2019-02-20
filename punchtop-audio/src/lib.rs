#![feature(proc_macro_hygiene, decl_macro)]
#![deny(clippy::all, clippy::pedantic)]

#[macro_use]
extern crate log;

use std::io::Read;
use std::time::Duration;

pub mod chromecast;

pub type Result = std::result::Result<(), Error>;

#[derive(Debug)]
pub enum Error {
    BackendNotInitialized,
    CannotLoadMedia,
}

#[derive(Debug, Default)]
pub struct Tags {
    pub artist: Option<String>,
    pub title: Option<String>,
    pub album: Option<String>,
}

#[derive(Debug)]
pub struct Image {
    pub bytes: Vec<u8>,
    pub mime: String,
    pub height: u32,
    pub width: u32,
}

pub trait Track {
    fn id(&self) -> &str;

    fn duration(&self) -> Duration;

    fn tags(&self) -> Option<Tags>;

    fn cover(&self) -> Option<Image>;

    fn stream(&self) -> Option<Box<dyn Read>>;

    fn content_type(&self) -> String;
}

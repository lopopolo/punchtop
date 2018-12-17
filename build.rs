extern crate protobuf_codegen_pure;

use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

#[derive(Debug)]
pub struct StrReplace {
    data: String,
}

impl StrReplace {
    pub fn from_file(path: &str) -> StrReplace {
        let filepath = Path::new(path);
        let mut file = File::open(filepath).unwrap();
        let mut data = String::new();

        file.read_to_string(&mut data)
            .expect("Failed to read file.");

        StrReplace { data }
    }

    pub fn replace(&mut self, search: &str, replacement: &str) -> &mut Self {
        self.data = self.data.replace(search, replacement);
        self
    }

    pub fn to_file(&self, dst: &str) {
        let mut file = File::create(dst).unwrap();
        file.write_all(self.data.as_bytes())
            .expect("Failed to write file.");
    }
}

fn main() {
    protobuf_codegen_pure::run(protobuf_codegen_pure::Args {
        out_dir: "src/cast/proto",
        input: &[
            "proto/cast/authority_keys.proto",
            "proto/cast/cast_channel.proto",
        ],
        includes: &["proto/cast"],
        customize: protobuf_codegen_pure::Customize {
            ..Default::default()
        },
    })
    .expect("protoc");

    let dest_path = Path::new(".").join("src/cast/proto/mod.rs");
    let mut f = File::create(&dest_path).unwrap();

    f.write_all(
        b"
        pub use self::authority_keys::*;
        pub use self::cast_channel::*;

        mod authority_keys;
        mod cast_channel;
    ",
    )
    .unwrap();

    StrReplace::from_file("src/cast/proto/authority_keys.rs")
        .replace(
            "#![allow(clippy)]",
            "#![allow(clippy::all, clippy::pedantic)]",
        )
        .to_file("src/cast/proto/authority_keys.rs");
    StrReplace::from_file("src/cast/proto/cast_channel.rs")
        .replace(
            "#![allow(clippy)]",
            "#![allow(clippy::all, clippy::pedantic)]",
        )
        .to_file("src/cast/proto/cast_channel.rs");
}

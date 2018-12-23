extern crate protobuf_codegen_pure;

use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

const MOD: &[u8] = b"
pub use self::authority_keys::*;
pub use self::cast_channel::*;

mod authority_keys;
mod cast_channel;
";

#[derive(Debug)]
pub struct StrReplace {
    data: String,
}

impl StrReplace {
    pub fn from_file(path: &str) -> StrReplace {
        let filepath = Path::new(path);
        let mut file = File::open(filepath).expect(&format!("Failed to open {}", path));
        let mut data = String::new();

        file.read_to_string(&mut data)
            .expect(&format!("Failed to read {}", path));

        StrReplace { data }
    }

    pub fn replace(&mut self, search: &str, replacement: &str) -> &mut Self {
        self.data = self.data.replace(search, replacement);
        self
    }

    pub fn to_file(&self, dst: &str) {
        let mut file = File::create(dst).expect(&format!("Failed to create {}", dst));
        file.write_all(self.data.as_bytes())
            .expect(&format!("Failed to write {}", dst));
    }
}

fn main() {
    let protos = &["proto/authority_keys.proto", "proto/cast_channel.proto"];
    let gen = &["src/proto/authority_keys.rs", "src/proto/cast_channel.rs"];

    protobuf_codegen_pure::run(protobuf_codegen_pure::Args {
        out_dir: "src/proto",
        input: protos,
        includes: &["proto"],
        customize: protobuf_codegen_pure::Customize {
            ..Default::default()
        },
    })
    .expect("protoc");

    for proto in gen {
        // Code mod to silence clippy warnings on nightly about deprecated lints
        StrReplace::from_file(proto)
            .replace(
                "#![allow(clippy)]",
                "#![allow(clippy::all, clippy::pedantic)]",
            )
            .to_file(proto);
    }

    let dest_path = Path::new(".").join("src/proto/mod.rs");
    let mut f = File::create(&dest_path).expect("Failed to create proto/mod.rs");
    f.write_all(MOD).expect("Failed to write proto/mod.rs");
}

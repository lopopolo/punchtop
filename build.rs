extern crate protobuf_codegen_pure;

use std::fs::File;
use std::io::Write;
use std::path::Path;

fn main() {
    protobuf_codegen_pure::run(protobuf_codegen_pure::Args {
        out_dir: "src/cast/proto",
        input: &["proto/cast/authority_keys.proto", "proto/cast/cast_channel.proto"],
        includes: &["proto/cast"],
        customize: protobuf_codegen_pure::Customize {
            ..Default::default()
        },
    }).expect("protoc");

    let dest_path = Path::new(".").join("src/cast/proto/mod.rs");
    let mut f = File::create(&dest_path).unwrap();

    f.write_all(b"
        pub use self::authority_keys::*;
        pub use self::cast_channel::*;

        mod authority_keys;
        mod cast_channel;
    ").unwrap();
}

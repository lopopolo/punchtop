extern crate protobuf_codegen_pure;

fn main() {
    protobuf_codegen_pure::run(protobuf_codegen_pure::Args {
        out_dir: "src/cast/proto",
        input: &["proto/cast/authority_keys.proto", "proto/cast/cast_channel.proto"],
        includes: &["proto/cast"],
        customize: protobuf_codegen_pure::Customize {
            ..Default::default()
        },
    }).expect("protoc");
}

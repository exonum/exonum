extern crate protoc_rust;

use protoc_rust::Customize;

fn main() {
    protoc_rust::run(protoc_rust::Args {
        out_dir: "src/proto",
        input: &["src/proto/cryptocurrency.proto"],
        includes: &["src/proto", "../../exonum/src/encoding/protobuf/proto"],
        customize: Customize {
            ..Default::default()
        },
    }).expect("protoc");
}

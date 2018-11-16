extern crate protoc_rust;

use protoc_rust::Customize;

fn main() {
    protoc_rust::run(protoc_rust::Args {
        out_dir: "src/proto",
        input: &["src/proto/time.proto", "src/proto/simple_service.proto"],
        includes: &["src/proto", "../../exonum/src/encoding/protobuf/proto"],
        customize: Customize {
            serde_derive: Some(true),
            ..Default::default()
        },
    }).expect("protoc");
}

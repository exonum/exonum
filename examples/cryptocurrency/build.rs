extern crate exonum_build;

use exonum_build::{protobuf_generate, ProtoSources};

fn main() {
    protobuf_generate(
        "src/proto",
        &[
            "src/proto".into(),
            ProtoSources::Exonum,
            ProtoSources::Crypto,
        ],
        "protobuf_mod.rs",
    );
}

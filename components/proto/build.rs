extern crate exonum_build;

use exonum_build::{protobuf_generate, ProtoSources};

fn main() {
    protobuf_generate(
        "src/proto",
        &[ProtoSources::Path("src/proto")],
        "protobuf_mod.rs",
    );
}

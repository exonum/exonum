extern crate exonum_build;

use exonum_build::{protobuf_generate, ProtoSources};
use std::env;

fn main() {
    let current_dir = env::current_dir().expect("Failed to get current dir.");
    let protos = current_dir.join("src/proto");
    println!("cargo:protos={}", protos.to_str().unwrap());

    protobuf_generate(
        "src/proto",
        &[ProtoSources::Path("src/proto")],
        "protobuf_mod.rs",
    );
}

extern crate exonum_build;

use exonum_build::ProtobufGenerator;
use std::env;

fn main() {
    let current_dir = env::current_dir().expect("Failed to get current dir.");
    let protos = current_dir.join("src/proto");
    println!("cargo:protos={}", protos.to_str().unwrap());

    ProtobufGenerator::with_mod_name("protobuf_mod.rs")
        .input_dir("src/proto")
        .add_path("src/proto")
        .generate();
}

extern crate exonum_build;

use exonum_build::ProtobufGenerator;

fn main() {
    ProtobufGenerator::with_mod_name("protobuf_mod.rs")
        .input_dir("src/proto")
        .add_path("src/proto")
        .generate();

    ProtobufGenerator::with_mod_name("simple_service_protobuf_mod.rs")
        .input_dir("examples/simple_service/proto")
        .add_path("examples/simple_service/proto")
        .generate();
}

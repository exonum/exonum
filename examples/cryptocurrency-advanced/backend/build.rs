use exonum_build::ProtobufGenerator;

fn main() {
    ProtobufGenerator::with_mod_name("protobuf_mod.rs")
        .input_dir("src/proto")
        .add_path("src/proto")
        .frequently_used()
        .generate();
}

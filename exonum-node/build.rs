use exonum_build::ProtobufGenerator;

fn main() {
    ProtobufGenerator::with_mod_name("exonum_node_mod.rs")
        .with_input_dir("src/proto")
        .with_common()
        .with_crypto()
        .with_exonum()
        .generate();
}

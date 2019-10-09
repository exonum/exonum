use exonum_build::{
    get_exonum_protobuf_crypto_files_path, get_exonum_protobuf_files_path, protobuf_generate,
};

fn main() {
    let exonum_protos = get_exonum_protobuf_files_path();
    let exonum_crypto_protos = get_exonum_protobuf_crypto_files_path();

    protobuf_generate(
        "src/proto",
        &[&exonum_protos, "src/proto", &exonum_crypto_protos],
        "protobuf_mod.rs",
    )
}

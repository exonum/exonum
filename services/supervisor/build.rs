extern crate exonum_build;

use exonum_build::{
    get_exonum_protobuf_crypto_files_path, get_exonum_protobuf_files_path, protobuf_generate,
};

fn main() {
    let exonum_protos = get_exonum_protobuf_files_path();
    let crypto_protos = get_exonum_protobuf_crypto_files_path();

    let protobuf_gen_data = [
        (
            "src/proto",
            vec!["src/proto", &exonum_protos, &crypto_protos],
            "protobuf_mod.rs",
        ),
        (
            "tests/supervisor/proto",
            vec!["tests/supervisor/proto", &crypto_protos],
            "supervisor_example_protobuf_mod.rs",
        ),
    ];

    for (input_dir, includes, mod_file_name) in protobuf_gen_data.into_iter() {
        protobuf_generate(input_dir, includes, mod_file_name);
    }
}

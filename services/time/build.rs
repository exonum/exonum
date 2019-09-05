extern crate exonum_build;

use exonum_build::{get_exonum_protobuf_files_path, protobuf_generate};

fn main() {
    let exonum_protos = get_exonum_protobuf_files_path();

    protobuf_generate("src/proto", &["src/proto", &exonum_protos], "protobuf_mod.rs");

    protobuf_generate(
        "examples/simple_service/proto",
        &["src/proto", "examples/simple_service/proto", &exonum_protos],
        "simple_service_protobuf_mod.rs",
    );
}

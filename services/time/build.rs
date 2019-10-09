extern crate exonum_build;

use exonum_build::protobuf_generate;

fn main() {
    protobuf_generate("src/proto", &["src/proto".into()], "protobuf_mod.rs");

    protobuf_generate(
        "examples/simple_service/proto",
        &["examples/simple_service/proto".into()],
        "simple_service_protobuf_mod.rs",
    );
}

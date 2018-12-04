extern crate exonum_build;

use exonum_build::protobuf_generate;
use std::env;

fn main() {
    let exonum_protos = env::var("DEP_EXONUM_PROTOBUF_PROTOS").unwrap();
    protobuf_generate(
        "src/proto",
        &["src/proto", &exonum_protos],
        "protobuf_mod.rs",
    );
}

use exonum_build::{protobuf_generate, ProtoSources};

fn main() {
    protobuf_generate(
        "src/proto",
        &[
            ProtoSources::Exonum,
            ProtoSources::Path("src/proto"),
            ProtoSources::Crypto,
        ],
        "protobuf_mod.rs",
    )
}

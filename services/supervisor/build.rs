extern crate exonum_build;

use exonum_build::{ProtoSources, ProtobufGenerator};

fn main() {
    let protobuf_gen_data = [
        (
            "src/proto",
            vec![
                "src/proto".into(),
                ProtoSources::Exonum,
                ProtoSources::Crypto,
            ],
            "protobuf_mod.rs",
        ),
        (
            "tests/supervisor/proto",
            vec!["tests/supervisor/proto".into(), ProtoSources::Crypto],
            "supervisor_example_protobuf_mod.rs",
        ),
    ];

    for (input_dir, includes, mod_file_name) in protobuf_gen_data.into_iter() {
        ProtobufGenerator::with_mod_name(mod_file_name)
            .input_dir(input_dir)
            .includes(includes)
            .generate();
    }
}

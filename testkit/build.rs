extern crate exonum_build;

use exonum_build::{ProtoSources, ProtobufGenerator};

fn main() {
    let protobuf_gen_data = [
        (
            "src/proto",
            vec!["src/proto".into()],
            "testkit_protobuf_mod.rs",
        ),
        (
            "src/simple_supervisor/proto",
            vec![
                "src/simple_supervisor/proto".into(),
                ProtoSources::Exonum,
                ProtoSources::Crypto,
            ],
            "simple_supervisor_mod.rs",
        ),
        (
            "tests/inflating_currency/proto",
            vec![
                "tests/inflating_currency/proto".into(),
                ProtoSources::Exonum,
                ProtoSources::Crypto,
            ],
            "currency_example_protobuf_mod.rs",
        ),
        (
            "tests/counter/proto",
            vec!["tests/counter/proto".into()],
            "counter_example_protobuf_mod.rs",
        ),
        (
            "tests/service_hooks/proto",
            vec!["tests/service_hooks/proto".into()],
            "hooks_example_protobuf_mod.rs",
        ),
        (
            "tests/interfaces/proto",
            vec![
                "tests/interfaces/proto".into(),
                ProtoSources::Exonum,
                ProtoSources::Crypto,
            ],
            "interfaces_protobuf_mod.rs",
        ),
        (
            "examples/timestamping/proto",
            vec!["examples/timestamping/proto".into()],
            "timestamping_example_protobuf_mod.rs",
        ),
    ];

    for (input_dir, includes, mod_file_name) in protobuf_gen_data.into_iter() {
        ProtobufGenerator::with_mod_name(mod_file_name)
            .input_dir(input_dir)
            .includes(includes)
            .generate();
    }
}

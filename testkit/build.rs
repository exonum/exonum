extern crate exonum_build;

use exonum_build::{get_exonum_protobuf_files_path, protobuf_generate};

fn main() {
    let exonum_protos = get_exonum_protobuf_files_path();
    let protobuf_gen_data = [
        ("src/proto", vec!["src/proto"], "testkit_protobuf_mod.rs"),
        (
            "tests/inflating_currency/proto",
            vec!["tests/inflating_currency/proto", &exonum_protos],
            "currency_example_protobuf_mod.rs",
        ),
        (
            "tests/counter/proto",
            vec!["tests/counter/proto"],
            "counter_example_protobuf_mod.rs",
        ),
        (
            "tests/service_hooks/proto",
            vec!["tests/service_hooks/proto"],
            "hooks_example_protobuf_mod.rs",
        ),
        (
            "examples/timestamping/proto",
            vec!["examples/timestamping/proto"],
            "timestamping_example_protobuf_mod.rs",
        ),
    ];

    for (input_dir, includes, mod_file_name) in protobuf_gen_data.iter() {
        protobuf_generate(input_dir, includes, mod_file_name);
    }
}

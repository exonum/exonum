extern crate exonum_build;

use exonum_build::protobuf_generate;

fn main() {
    protobuf_generate("src/proto", &["src/proto"], "testkit_protobuf_mod.rs");

    let exonum_protos = exonum_build::get_exonum_protobuf_files_path();
    protobuf_generate(
        "tests/inflating_currency/proto",
        &["tests/inflating_currency/proto", &exonum_protos],
        "currency_example_protobuf_mod.rs",
    );

    protobuf_generate(
        "tests/counter/proto",
        &["tests/counter/proto"],
        "counter_example_protobuf_mod.rs",
    );

    protobuf_generate(
        "tests/service_hooks",
        &["tests/service_hooks"],
        "hooks_example_protobuf_mod.rs",
    );

    protobuf_generate(
        "examples/timestamping/proto",
        &["examples/timestamping/proto"],
        "timestamping_example_protobuf_mod.rs",
    );
}

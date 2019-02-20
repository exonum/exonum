// spell-checker:ignore rustc

extern crate exonum_build;

use exonum_build::protobuf_generate;

use std::{env, fs::File, io::Write, path::Path, process::Command};

static USER_AGENT_FILE_NAME: &str = "user_agent";

fn create_path_to_protobuf_schema_env() {
    // Workaround for https://github.com/rust-lang/cargo/issues/3544
    // We "link" exonum with exonum_protobuf library
    // and dependents in their `build.rs` will have access to `$DEP_EXONUM_PROTOBUF_PROTOS`.
    let path = env::current_dir()
        .expect("Failed to get current dir.")
        .join("src/proto/schema/exonum");
    println!("cargo:protos={}", path.to_str().unwrap());
}

fn write_user_agent_file() {
    let package_name = option_env!("CARGO_PKG_NAME").unwrap_or("exonum");
    let package_version = option_env!("CARGO_PKG_VERSION").unwrap_or("?");
    let rust_version = rust_version().unwrap_or("rust ?".to_string());
    let user_agent = format!("{} {}/{}\n", package_name, package_version, rust_version);

    let out_dir = env::var("OUT_DIR").expect("Unable to get OUT_DIR");
    let dest_path = Path::new(&out_dir).join(USER_AGENT_FILE_NAME);
    let mut file = File::create(dest_path).expect("Unable to create output file");
    file.write_all(user_agent.as_bytes())
        .expect("Unable to write data to file");
}

fn main() {
    write_user_agent_file();

    create_path_to_protobuf_schema_env();

    protobuf_generate(
        "src/proto/schema/exonum",
        &["src/proto/schema/exonum"],
        "exonum_proto_mod.rs",
    );

    // Exonum external tests.
    protobuf_generate(
        "tests/explorer/blockchain/proto",
        &["tests/explorer/blockchain/proto", "src/proto/schema/exonum"],
        "exonum_tests_proto_mod.rs",
    );

    // Exonum benchmarks.
    protobuf_generate(
        "benches/criterion/proto",
        &["benches/criterion/proto", "src/proto/schema/exonum"],
        "exonum_benches_proto_mod.rs",
    );
}

fn rust_version() -> Option<String> {
    let rustc = option_env!("RUSTC").unwrap_or("rustc");

    let output = Command::new(rustc).arg("-V").output().ok()?.stdout;
    String::from_utf8(output).ok()
}

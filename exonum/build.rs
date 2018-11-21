// spell-checker:ignore rustc, walkdir

extern crate protoc_rust;
extern crate walkdir;

use protoc_rust::Customize;
use walkdir::WalkDir;

use std::{env, fs::File, io::Write, path::Path, process::Command};

static USER_AGENT_FILE_NAME: &str = "user_agent";

fn get_proto_files<P: AsRef<Path>>(path: P) -> Vec<String> {
    WalkDir::new(path)
        .into_iter()
        .filter_map(|e| {
            let e = e.ok()?;
            if e.path().extension()?.to_str() == Some("proto") {
                Some(e.path().to_str()?.to_owned())
            } else {
                None
            }
        }).collect()
}

fn protoc_generate(out_dir: &str, input_dir: &str, includes: &[&str]) {
    protoc_rust::run(protoc_rust::Args {
        out_dir,
        input: &get_proto_files(input_dir)
            .iter()
            .map(|s| s.as_ref())
            .collect::<Vec<_>>(),
        includes,
        customize: Customize {
            serde_derive: Some(true),
            ..Default::default()
        },
    }).expect("protoc");
}

fn main() {
    let package_name = option_env!("CARGO_PKG_NAME").unwrap_or("exonum");
    let package_version = option_env!("CARGO_PKG_VERSION").unwrap_or("?");
    let rust_version = rust_version().unwrap_or("rust ?".to_string());
    let user_agent = format!("{} {}/{}\n", package_name, package_version, rust_version);

    let out_dir = env::var("OUT_DIR").expect("Unable to get OUT_DIR");
    let dest_path = Path::new(&out_dir).join(USER_AGENT_FILE_NAME);
    let mut file = File::create(dest_path).expect("Unable to create output file");
    file.write_all(user_agent.as_bytes())
        .expect("Unable to data to file");

    protoc_generate(
        "src/encoding/protobuf",
        "src/encoding/protobuf/proto/",
        &["src/encoding/protobuf/proto"],
    );

    // Exonum external tests.
    protoc_generate(
        "tests/explorer/blockchain/proto",
        "tests/explorer/blockchain/proto",
        &[
            "tests/explorer/blockchain/proto",
            "src/encoding/protobuf/proto",
        ],
    );

    // Exonum benchmarks.
    protoc_generate(
        "benches/criterion/proto",
        "benches/criterion/proto",
        &["benches/criterion", "src/encoding/protobuf/proto"],
    );
}

fn rust_version() -> Option<String> {
    let rustc = option_env!("RUSTC").unwrap_or("rustc");

    let output = Command::new(rustc).arg("-V").output().ok()?.stdout;
    String::from_utf8(output).ok()
}

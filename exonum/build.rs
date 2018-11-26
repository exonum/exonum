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

/// Workaround for https://github.com/stepancheg/rust-protobuf/issues/324
fn generate_mod_rs(out_dir: &str, proto_files: &[String], mod_file: &str) {
    let mod_file_content = {
        proto_files
            .iter()
            .map(|f| {
                let mod_name = Path::new(f)
                    .file_stem()
                    .unwrap()
                    .to_str()
                    .expect("proto file name is not &str");
                if mod_name == "tests" {
                    format!("#[cfg(test)]\npub mod {};\n", mod_name)
                } else {
                    format!("pub mod {};\n", mod_name)
                }
            }).collect::<String>()
    };
    let dest_path = Path::new(&out_dir).join(mod_file);
    let mut file = File::create(dest_path).expect("Unable to create output file");
    file.write_all(mod_file_content.as_bytes())
        .expect("Unable to write data to file");
}

fn protoc_generate(out_dir: &str, input_dir: &str, includes: &[&str], mod_file: &str) {
    let proto_files = get_proto_files(input_dir);

    generate_mod_rs(out_dir, &proto_files, mod_file);

    protoc_rust::run(protoc_rust::Args {
        out_dir,
        input: &proto_files.iter().map(|s| s.as_ref()).collect::<Vec<_>>(),
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
        .expect("Unable to write data to file");

    protoc_generate(
        &out_dir,
        "src/encoding/protobuf/proto/",
        &["src/encoding/protobuf/proto"],
        "exonum_proto_mod.rs",
    );

    // Exonum external tests.
    protoc_generate(
        &out_dir,
        "tests/explorer/blockchain/proto",
        &[
            "tests/explorer/blockchain/proto",
            "src/encoding/protobuf/proto",
        ],
        "exonum_tests_proto_mod.rs",
    );

    // Exonum benchmarks.
    protoc_generate(
        &out_dir,
        "benches/criterion/proto",
        &["benches/criterion", "src/encoding/protobuf/proto"],
        "exonum_benches_proto_mod.rs",
    );
}

fn rust_version() -> Option<String> {
    let rustc = option_env!("RUSTC").unwrap_or("rustc");

    let output = Command::new(rustc).arg("-V").output().ok()?.stdout;
    String::from_utf8(output).ok()
}

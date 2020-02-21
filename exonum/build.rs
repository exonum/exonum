// Copyright 2020 The Exonum Team
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

// spell-checker:ignore rustc

use exonum_build::{ProtoSources, ProtobufGenerator};

use std::{env, fs::File, io::Write, path::Path, process::Command};

static USER_AGENT_FILE_NAME: &str = "user_agent";

fn create_path_to_protobuf_schema_env() {
    // Workaround for https://github.com/rust-lang/cargo/issues/3544
    // We "link" exonum with exonum_protobuf library
    // and dependents in their `build.rs` will have access to `$DEP_EXONUM_PROTOBUF_PROTOS`.

    let current_dir = env::current_dir().expect("Failed to get current dir.");
    let protos = current_dir.join("src/proto/schema");
    println!("cargo:protos={}", protos.to_str().unwrap());

    // Reexport common, MerkleDB and crypto protobuf files.
    let common_protos = env::var("DEP_EXONUM_PROTOBUF_COMMON_PROTOS")
        .expect("Cannot obtain `common` protobuf files");
    println!("cargo:common_protos={}", common_protos);
    let crypto_protos = env::var("DEP_EXONUM_PROTOBUF_CRYPTO_PROTOS")
        .expect("Cannot obtain `crypto` protobuf files");
    println!("cargo:crypto_protos={}", crypto_protos);
    let merkledb_protos = env::var("DEP_EXONUM_PROTOBUF_MERKLEDB_PROTOS")
        .expect("Cannot obtain `merkledb` protobuf files");
    println!("cargo:merkledb_protos={}", merkledb_protos);
}

fn write_user_agent_file() {
    let exonum_version = option_env!("CARGO_PKG_VERSION").unwrap_or("?");
    let rust_version = rust_version().unwrap_or_else(|| "0.0.0".to_string());
    let user_agent = format!("{}/{}\n", exonum_version, rust_version);

    let out_dir = env::var("OUT_DIR").expect("Unable to get OUT_DIR");
    let dest_path = Path::new(&out_dir).join(USER_AGENT_FILE_NAME);
    let mut file = File::create(dest_path).expect("Unable to create output file");
    file.write_all(user_agent.as_bytes())
        .expect("Unable to write data to file");
}

fn main() {
    write_user_agent_file();
    create_path_to_protobuf_schema_env();

    ProtobufGenerator::with_mod_name("exonum_proto_mod.rs")
        .with_input_dir("src/proto/schema")
        .with_crypto()
        .with_common()
        .with_merkledb()
        .generate();

    ProtobufGenerator::with_mod_name("exonum_details_mod.rs")
        .with_input_dir("src/proto/details")
        .with_crypto()
        .with_includes(&[ProtoSources::Path("src/proto/schema")])
        .without_sources()
        .generate();
}

fn rust_version() -> Option<String> {
    let rustc = option_env!("RUSTC").unwrap_or("rustc");
    let output = Command::new(rustc).arg("-V").output().ok()?.stdout;
    let rustc_output = std::str::from_utf8(&output).ok()?;
    let version = rustc_output.split_whitespace().nth(1)?;
    Some(version.to_owned())
}

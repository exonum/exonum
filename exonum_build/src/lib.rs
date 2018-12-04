// Copyright 2018 The Exonum Team
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

// spell-checker:ignore walkdir, subfolders, submodules

//! This crate simplifies writing build.rs for exonum and exonum services.

extern crate protoc_rust;
extern crate walkdir;

use protoc_rust::Customize;
use walkdir::WalkDir;

use std::{
    env,
    fs::File,
    io::Write,
    path::{Path, PathBuf},
};

/// Finds all .proto files in `path` and subfolders and returns a vector of their paths.
fn get_proto_files<P: AsRef<Path>>(path: &P) -> Vec<PathBuf> {
    WalkDir::new(path)
        .into_iter()
        .filter_map(|e| {
            let e = e.ok()?;
            if e.path().extension()?.to_str() == Some("proto") {
                Some(e.path().into())
            } else {
                None
            }
        }).collect()
}

/// Workaround for https://github.com/stepancheg/rust-protobuf/issues/324
/// It is impossible to `include!` .rs files generated by rust-protobuf
/// so we generate piece of `mod.rs` which includes generated files as public submodules.
///
/// tests.proto .rs file will be included with `#[cfg(test)]`.
fn generate_mod_rs<P: AsRef<Path>, Q: AsRef<Path>>(
    out_dir: &P,
    proto_files: &[PathBuf],
    mod_file: &Q,
) {
    let mod_file_content = {
        proto_files
            .iter()
            .map(|f| {
                let mod_name = f
                    .file_stem()
                    .unwrap()
                    .to_str()
                    .expect(".proto file name is not convertible to &str");
                if mod_name == "tests" {
                    format!("#[cfg(test)]\npub mod {};\n", mod_name)
                } else {
                    format!("pub mod {};\n", mod_name)
                }
            }).collect::<String>()
    };
    let dest_path = out_dir.as_ref().join(mod_file);
    let mut file = File::create(dest_path).expect("Unable to create output file");
    file.write_all(mod_file_content.as_bytes())
        .expect("Unable to write data to file");
}

/// Generates .rs files from .proto files.
///
/// `protoc` executable from protobuf should be in `$PATH`
///
/// # Examples
///
/// In `build.rs`
/// ```no_run
/// extern crate exonum_build;
///
/// use exonum_build::protobuf_generate;
///
/// // Includes usually should contain input_dir.
/// protobuf_generate("src/proto", &["src/proto"], "example_mod.rs")
/// ```
/// After successful run `$OUT_DIR` will contain \*.rs for each \*.proto file in
/// "src/proto/\*\*/" and example_mod.rs which will include all generated .rs files
/// as submodules.
///
/// To use generated protobuf structs.
///
/// In `src/proto/mod.rs`
/// ```ignore
/// extern crate exonum;
///
/// include!(concat!(env!("OUT_DIR"), "/example_mod.rs"));
///
/// // If you use types from exonum .proto files.
/// use exonum::encoding::protobuf::*;
/// ```
pub fn protobuf_generate<P, R, I, T>(input_dir: P, includes: I, mod_file_name: T)
where
    P: AsRef<Path>,
    R: AsRef<Path>,
    I: IntoIterator<Item = R>,
    T: AsRef<str>,
{
    let out_dir = env::var("OUT_DIR")
        .map(PathBuf::from)
        .expect("Unable to get OUT_DIR");

    let proto_files = get_proto_files(&input_dir);
    generate_mod_rs(&out_dir, &proto_files, &mod_file_name.as_ref());

    let includes = includes.into_iter().collect::<Vec<_>>();

    protoc_rust::run(protoc_rust::Args {
        out_dir: out_dir
            .to_str()
            .expect("Out dir name is not convertible to &str"),
        input: &proto_files
            .iter()
            .map(|s| s.to_str().expect("File name is not convertible to &str"))
            .collect::<Vec<_>>(),
        includes: &includes
            .iter()
            .map(|s| {
                s.as_ref()
                    .to_str()
                    .expect("Include dir name is not convertible to &str")
            }).collect::<Vec<_>>(),
        customize: Customize {
            serde_derive: Some(true),
            ..Default::default()
        },
    }).expect("protoc");

    // rerun build.rs if .proto files changed.
    println!(
        "cargo:rerun-if-changed={}",
        input_dir
            .as_ref()
            .to_str()
            .expect("Input dir name is not convertible to &str")
    );
}

pub fn get_exonum_protobuf_files_path() -> String {
    env::var("DEP_EXONUM_PROTOBUF_PROTOS").expect("Failed to get exonum protobuf path")
}

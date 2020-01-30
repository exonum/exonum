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

//! This crate simplifies writing build scripts (`build.rs`) for Exonum and Exonum services.
//!
//! Since Protobuf is the Exonum default serialization format, `build.rs` is mostly used
//! to compile Protobuf files and generate a corresponding code. This code is used later by
//! the Exonum core and services.
//!
//! In order to use the crate, call `ProtobufGenerator` with the required params.
//! See [`ProtobufGenerator`] docs for an example.
//!
//! # File Sets
//!
//! There are several predefined sets of Protobuf sources available for use, split according
//! to the crate the sources are defined in. These sets are described by [`ProtoSources`]:
//!
//! - **Crypto sources:** cryptographic types used in services and the code.
//! - **Common sources:** types that can be used by various parts of Exonum.
//! - **MerkleDB sources:** types representing proofs of existence of element in database.
//! - **Core sources:** types used in core and in system services such as supervisor.
//!
//! | File path | Set | Description |
//! |-----------|-----|-------------|
//! | `exonum/crypto/types.proto` | Crypto | Basic types: `Hash`, `PublicKey` and `Signature` |
//! | `exonum/common/bit_vec.proto` | Common | Protobuf mapping for `BitVec` |
//! | `exonum/proof/list_proof.proto` | MerkleDB | `ListProof` and related helpers |
//! | `exonum/proof/map_proof.proto` | MerkleDB | `MapProof` and related helpers |
//! | `exonum/blockchain.proto` | Core | Basic core types (e.g., `Block`) |
//! | `exonum/key_value_sequence.proto` | Core | Key-value sequence used to store additional headers in `Block` |
//! | `exonum/messages.proto` | Core | Base types for Ed25519-authenticated messages |
//! | `exonum/proofs.proto` | Core | Block and index proofs |
//! | `exonum/runtime/auth.proto` | Core | Authorization-related types |
//! | `exonum/runtime/base.proto` | Core | Basic runtime types (e.g., artifact ID) |
//! | `exonum/runtime/errors.proto` | Core | Execution errors |
//! | `exonum/runtime/lifecycle.proto` | Core | Advanced types used in service lifecycle |
//!
//! Each file is placed in the Protobuf package matching its path, similar to well-known Protobuf
//! types. For example, `exonum/runtime/auth.proto` types are in the `exonum.runtime` package.
//!
//! [`ProtobufGenerator`]: struct.ProtobufGenerator.html
//! [`ProtoSources`]: enum.ProtoSources.html

#![deny(unsafe_code, bare_trait_objects)]
#![warn(missing_docs, missing_debug_implementations)]

use proc_macro2::{Ident, Span, TokenStream};
use protoc_rust::Customize;
use quote::{quote, ToTokens};
use walkdir::WalkDir;

use std::collections::HashSet;
use std::{
    env,
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
};

/// Enum representing various sources of Protobuf files.
#[derive(Debug, Copy, Clone)]
pub enum ProtoSources<'a> {
    /// Path to core Protobuf files.
    Exonum,
    /// Path to crypto Protobuf files.
    Crypto,
    /// Path to common Protobuf files.
    Common,
    /// Path to database-related Protobuf files.
    Merkledb,
    /// Manually specified path.
    Path(&'a str),
}

impl<'a> ProtoSources<'a> {
    /// Returns path to protobuf files.
    pub fn path(&self) -> String {
        match self {
            ProtoSources::Exonum => get_exonum_protobuf_files_path(),
            ProtoSources::Common => get_exonum_protobuf_common_files_path(),
            ProtoSources::Crypto => get_exonum_protobuf_crypto_files_path(),
            ProtoSources::Merkledb => get_exonum_protobuf_merkledb_files_path(),
            ProtoSources::Path(path) => (*path).to_string(),
        }
    }
}

impl<'a> From<&'a str> for ProtoSources<'a> {
    fn from(path: &'a str) -> Self {
        ProtoSources::Path(path)
    }
}

#[derive(Debug, PartialEq, Eq, Hash)]
struct ProtobufFile {
    full_path: PathBuf,
    relative_path: String,
}

/// Finds all .proto files in `path` and sub-directories and returns a vector
/// with metadata on found files.
fn get_proto_files<P: AsRef<Path>>(path: &P) -> Vec<ProtobufFile> {
    WalkDir::new(path)
        .into_iter()
        .filter_map(|e| {
            let entry = e.ok()?;
            if entry.file_type().is_file() && entry.path().extension()?.to_str() == Some("proto") {
                let full_path = entry.path().to_owned();
                let relative_path = full_path.strip_prefix(path).unwrap().to_owned();
                let relative_path = relative_path
                    .to_str()
                    .expect("Cannot convert relative path to string");

                Some(ProtobufFile {
                    full_path,
                    relative_path: canonicalize_protobuf_path(&relative_path),
                })
            } else {
                None
            }
        })
        .collect()
}

#[cfg(windows)]
fn canonicalize_protobuf_path(path_str: &str) -> String {
    path_str.replace('\\', "/")
}

#[cfg(not(windows))]
fn canonicalize_protobuf_path(path_str: &str) -> String {
    path_str.to_owned()
}

/// Includes all .proto files with their names into generated file as array of tuples,
/// where tuple content is (file_name, file_content).
fn include_proto_files(proto_files: HashSet<&ProtobufFile>, name: &str) -> impl ToTokens {
    let proto_files_len = proto_files.len();
    // TODO Think about syn crate and token streams instead of dirty strings.
    let proto_files = proto_files.iter().map(|file| {
        let name = &file.relative_path;

        let mut content = String::new();
        File::open(&file.full_path)
            .expect("Unable to open .proto file")
            .read_to_string(&mut content)
            .expect("Unable to read .proto file");

        quote! {
            (#name, #content),
        }
    });

    let name = Ident::new(name, Span::call_site());

    quote! {
        /// Original proto files which were be used to generate this module.
        /// First element in tuple is file name, second is proto file content.
        #[allow(dead_code)]
        #[allow(clippy::unseparated_literal_suffix)]
        pub const #name: [(&str, &str); #proto_files_len] = [
            #( #proto_files )*
        ];
    }
}

fn get_mod_files(proto_files: &[ProtobufFile]) -> impl Iterator<Item = TokenStream> + '_ {
    proto_files.iter().map(|file| {
        let mod_name = file
            .full_path
            .file_stem()
            .unwrap()
            .to_str()
            .expect(".proto file name is not convertible to &str");

        let mod_name = Ident::new(mod_name, Span::call_site());
        if mod_name == "tests" {
            quote! {
                #[cfg(test)] pub mod #mod_name;
            }
        } else {
            quote! {
                pub mod #mod_name;
            }
        }
    })
}

/// Collects .rs files generated by the rust-protobuf into single module.
///
/// - If module name is `tests` it adds `#[cfg(test)]` to declaration.
/// - Also this method includes source files as `PROTO_SOURCES` constant.
fn generate_mod_rs(
    out_dir: impl AsRef<Path>,
    proto_files: &[ProtobufFile],
    includes: &[ProtobufFile],
    mod_file: impl AsRef<Path>,
) {
    let mod_files = get_mod_files(proto_files);

    // To avoid cases where input sources are also added as includes, use only
    // unique paths.
    let includes = includes
        .iter()
        .filter(|file| !proto_files.contains(file))
        .collect();

    let proto_files = include_proto_files(proto_files.iter().collect(), "PROTO_SOURCES");
    let includes = include_proto_files(includes, "INCLUDES");

    let content = quote! {
        #( #mod_files )*
        #proto_files
        #includes
    };

    let dest_path = out_dir.as_ref().join(mod_file);
    let mut file = File::create(dest_path).expect("Unable to create output file");
    file.write_all(content.into_token_stream().to_string().as_bytes())
        .expect("Unable to write data to file");
}

fn generate_mod_rs_without_sources(
    out_dir: impl AsRef<Path>,
    proto_files: &[ProtobufFile],
    mod_file: impl AsRef<Path>,
) {
    let mod_files = get_mod_files(proto_files);
    let content = quote! {
        #( #mod_files )*
    };
    let dest_path = out_dir.as_ref().join(mod_file);
    let mut file = File::create(dest_path).expect("Unable to create output file");
    file.write_all(content.into_token_stream().to_string().as_bytes())
        .expect("Unable to write data to file");
}

/// Generates Rust modules from Protobuf files.
///
/// The `protoc` executable (i.e., the Protobuf compiler) should be in `$PATH`.
///
/// # Examples
///
/// Specify in the build script (`build.rs`) of your crate:
///
/// ```no_run
/// use exonum_build::ProtobufGenerator;
///
/// ProtobufGenerator::with_mod_name("example_mod.rs")
///     .with_input_dir("src/proto")
///     .with_crypto()
///     .with_common()
///     .with_merkledb()
///     .generate();
/// ```
///
/// After the successful run, `$OUT_DIR` will contain a module for each Protobuf file in
/// `src/proto` and `example_mod.rs` which will include all generated modules
/// as submodules.
///
/// To use the generated Rust types corresponding to Protobuf messages, specify
/// in `src/proto/mod.rs`:
///
/// ```ignore
/// include!(concat!(env!("OUT_DIR"), "/example_mod.rs"));
///
/// // If you use types from `exonum` .proto files.
/// use exonum::proto::schema::*;
/// ```
#[derive(Debug)]
pub struct ProtobufGenerator<'a> {
    includes: Vec<ProtoSources<'a>>,
    mod_name: &'a str,
    input_dir: &'a str,
    include_sources: bool,
}

impl<'a> ProtobufGenerator<'a> {
    /// Name of the rust module generated from input proto files.
    ///
    /// # Panics
    ///
    /// If the `mod_name` is empty.
    pub fn with_mod_name(mod_name: &'a str) -> Self {
        assert!(!mod_name.is_empty(), "Mod name is not specified");
        Self {
            includes: Vec::new(),
            input_dir: "",
            mod_name,
            include_sources: true,
        }
    }

    /// A directory containing input protobuf files.
    /// For single `mod_name` you can provide only one input directory,
    /// If proto-files in the input directory have dependencies located in another
    /// directories, you must specify them using `add_path` method.
    ///
    /// Predefined dependencies can be specified using corresponding methods
    /// `with_common`, `with_crypto`, `with_exonum`.
    ///
    /// # Panics
    ///
    /// If the input directory is already specified.
    pub fn with_input_dir(mut self, path: &'a str) -> Self {
        assert!(
            self.input_dir.is_empty(),
            "Input directory is already specified"
        );
        self.input_dir = path;
        self.includes.push(ProtoSources::Path(path));
        self
    }

    /// An additional directory containing dependent proto-files, can be used
    /// multiple times.
    pub fn add_path(mut self, path: &'a str) -> Self {
        self.includes.push(ProtoSources::Path(path));
        self
    }

    /// Common types for all crates.
    pub fn with_common(mut self) -> Self {
        self.includes.push(ProtoSources::Common);
        self
    }

    /// Proto files from `exonum-crypto` crate (`Hash`, `PublicKey`, etc.).
    pub fn with_crypto(mut self) -> Self {
        self.includes.push(ProtoSources::Crypto);
        self
    }

    /// Proto files from `exonum-merkledb` crate (`MapProof`, `ListProof`).
    pub fn with_merkledb(mut self) -> Self {
        self.includes.push(ProtoSources::Merkledb);
        self
    }

    /// Exonum core related proto files,
    pub fn with_exonum(mut self) -> Self {
        self.includes.push(ProtoSources::Exonum);
        self
    }

    /// Add multiple include directories.
    pub fn with_includes(mut self, includes: &'a [ProtoSources<'_>]) -> Self {
        self.includes.extend_from_slice(includes);
        self
    }

    /// Switches off inclusion of source Protobuf files into the generated output.
    pub fn without_sources(mut self) -> Self {
        self.include_sources = false;
        self
    }

    /// Generate proto files from specified sources.
    ///
    /// # Panics
    ///
    /// If the `input_dir` or `includes` are empty.
    pub fn generate(self) {
        assert!(!self.input_dir.is_empty(), "Input dir is not specified");
        assert!(!self.includes.is_empty(), "Includes are not specified");
        protobuf_generate(
            self.input_dir,
            &self.includes,
            self.mod_name,
            self.include_sources,
        );
    }
}

fn protobuf_generate(
    input_dir: &str,
    includes: &[ProtoSources<'_>],
    mod_file_name: &str,
    include_sources: bool,
) {
    let out_dir = env::var("OUT_DIR")
        .map(PathBuf::from)
        .expect("Unable to get OUT_DIR");

    // Converts paths to strings and adds input dir to includes.
    let mut includes: Vec<_> = includes.iter().map(ProtoSources::path).collect();
    includes.push(input_dir.to_owned());
    let includes: Vec<&str> = includes.iter().map(String::as_str).collect();

    let proto_files = get_proto_files(&input_dir);
    if include_sources {
        let included_files = get_included_files(&includes);
        generate_mod_rs(&out_dir, &proto_files, &included_files, mod_file_name);
    } else {
        generate_mod_rs_without_sources(&out_dir, &proto_files, mod_file_name);
    }

    protoc_rust::run(protoc_rust::Args {
        out_dir: out_dir
            .to_str()
            .expect("Out dir name is not convertible to &str"),
        input: &proto_files
            .iter()
            .map(|s| {
                s.full_path
                    .to_str()
                    .expect("File name is not convertible to &str")
            })
            .collect::<Vec<_>>(),
        includes: &includes,
        customize: Customize {
            serde_derive: Some(true),
            ..Default::default()
        },
    })
    .expect("protoc");
}

fn get_included_files(includes: &[&str]) -> Vec<ProtobufFile> {
    includes
        .iter()
        .flat_map(|path| get_proto_files(path))
        .collect()
}

/// Get path to the folder containing `exonum` protobuf files.
///
/// Needed for code generation of .proto files which import `exonum` provided .proto files.
fn get_exonum_protobuf_files_path() -> String {
    env::var("DEP_EXONUM_PROTOBUF_PROTOS").expect("Failed to get exonum protobuf path")
}

/// Get path to the folder containing `exonum-crypto` protobuf files.
fn get_exonum_protobuf_crypto_files_path() -> String {
    env::var("DEP_EXONUM_PROTOBUF_CRYPTO_PROTOS")
        .expect("Failed to get exonum crypto protobuf path")
}

/// Get path to the folder containing `exonum-proto` protobuf files.
fn get_exonum_protobuf_common_files_path() -> String {
    env::var("DEP_EXONUM_PROTOBUF_COMMON_PROTOS")
        .expect("Failed to get exonum common protobuf path")
}

/// Get path to the folder containing `exonum-merkledb` protobuf files.
fn get_exonum_protobuf_merkledb_files_path() -> String {
    env::var("DEP_EXONUM_PROTOBUF_MERKLEDB_PROTOS")
        .expect("Failed to get exonum merkledb protobuf path")
}

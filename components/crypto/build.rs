// Copyright 2019 The Exonum Team
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

//! Common macros for crypto module.

extern crate exonum_build;

use exonum_build::protobuf_generate;

fn main() {
    gen_proto_files();
}

#[cfg(feature = "protobuf_serialization")]
fn gen_proto_files() {
    protobuf_generate("src/proto", &["src/proto"], "protobuf_mod.rs");
}

#[cfg(not(feature = "protobuf_serialization"))]
fn gen_proto_files() {}

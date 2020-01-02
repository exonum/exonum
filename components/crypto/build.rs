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

use std::env;

use exonum_build::ProtobufGenerator;

fn main() {
    #[cfg(feature = "with-protobuf")]
    gen_proto_files();
}

#[cfg(feature = "with-protobuf")]
fn gen_proto_files() {
    let current_dir = env::current_dir().expect("Failed to get current dir.");
    let protos = current_dir.join("src/proto/schema");
    println!("cargo:protos={}", protos.to_str().unwrap());

    ProtobufGenerator::with_mod_name("protobuf_mod.rs")
        .with_input_dir("src/proto")
        .generate();
}

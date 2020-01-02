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

use exonum_build::{ProtoSources, ProtobufGenerator};

fn main() {
    let protobuf_gen_data = [
        (
            "tests/inflating_currency/proto",
            vec![
                "tests/inflating_currency/proto".into(),
                ProtoSources::Exonum,
                ProtoSources::Crypto,
            ],
            "currency_example_protobuf_mod.rs",
        ),
        (
            "tests/runtime_api/proto",
            vec!["tests/runtime_api/proto".into()],
            "test_runtime_api_protobuf_mod.rs",
        ),
        (
            "tests/interfaces/proto",
            vec![
                "tests/interfaces/proto".into(),
                ProtoSources::Exonum,
                ProtoSources::Crypto,
            ],
            "interfaces_protobuf_mod.rs",
        ),
    ];

    for (input_dir, includes, mod_file_name) in protobuf_gen_data.into_iter() {
        ProtobufGenerator::with_mod_name(mod_file_name)
            .with_input_dir(input_dir)
            .with_includes(includes)
            .generate();
    }
}

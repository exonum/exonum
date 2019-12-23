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

use exonum_build::ProtobufGenerator;

fn main() {
    // Exonum benchmarks.
    ProtobufGenerator::with_mod_name("exonum_benches_proto_mod.rs")
        .with_input_dir("benches/criterion/proto")
        .with_crypto()
        .with_common()
        .generate();
}

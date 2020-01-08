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

#![no_main]
#[macro_use]
extern crate libfuzzer_sys;
extern crate exonum;

use exonum::messages::RawMessage;

fn fuzz_target(data: &[u8]) {
    let msg = RawMessage::from_vec(data.to_vec());

    let _ = msg.version();
    let _ = msg.service_id();
    let _ = msg.message_type();
    let _ = msg.body();
    let _ = msg.signature();
}

fuzz_target!(|data| {
    fuzz_target(data);
});

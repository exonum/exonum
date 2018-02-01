// Copyright 2017 The Exonum Team
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

use router::Router;
use serde_json;

use blockchain::{Block, SCHEMA_MAJOR_VERSION};
use crypto::Hash;
use helpers::{Height, ValidatorId};

use super::*;

#[test]
fn test_json_response_for_complex_val() {
    let str_val = "sghdkgskgskldghshgsd";
    let txs = [34, 32];
    let tx_count = txs.len() as u32;
    let complex_val = Block::new(
        SCHEMA_MAJOR_VERSION,
        ValidatorId::zero(),
        Height(24),
        tx_count,
        &Hash::new([24; 32]),
        &Hash::new([34; 32]),
        &Hash::new([38; 32]),
    );
    struct SampleAPI;
    impl Api for SampleAPI {
        fn wire<'b>(&self, _: &'b mut Router) {
            return;
        }
    }
    let stub = SampleAPI;
    let result = stub.ok_response(&serde_json::to_value(str_val).unwrap());
    assert!(result.is_ok());
    let result = stub.ok_response(&serde_json::to_value(&complex_val).unwrap());
    assert!(result.is_ok());
    print!("{:?}", result);
}

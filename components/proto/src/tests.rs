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

// cspell:ignore AQIDBA

use bit_vec::BitVec;
use rand::{rngs::StdRng, Rng, SeedableRng};
use serde_json::json;

use crate::{ProtobufBase64, ProtobufConvert};

#[test]
fn test_bitvec_pb_convert() {
    let bv = BitVec::from_bytes(&[0b_1010_0000, 0b_0001_0010]);

    let pb_bv = bv.to_pb();
    let pb_round_trip: BitVec = ProtobufConvert::from_pb(pb_bv).unwrap();
    assert_eq!(pb_round_trip, bv);
}

#[derive(Debug, Serialize, Deserialize)]
struct Test {
    #[serde(with = "ProtobufBase64")]
    bytes: Vec<u8>,
}

#[test]
fn base64_serialization() {
    let mut test = Test {
        bytes: vec![1, 2, 3, 4],
    };
    let obj = serde_json::to_value(&test).unwrap();
    assert_eq!(obj, json!({ "bytes": "AQIDBA" }));

    test.bytes = vec![255, 255];
    let obj = serde_json::to_value(&test).unwrap();
    assert_eq!(obj, json!({ "bytes": "//8" }));
}

#[test]
fn base64_deserialization() {
    let test: Test = serde_json::from_value(json!({ "bytes": "//8=" })).unwrap();
    assert_eq!(test.bytes, &[255, 255]);
    let test: Test = serde_json::from_value(json!({ "bytes": "//8" })).unwrap();
    assert_eq!(test.bytes, &[255, 255]);
    let test: Test = serde_json::from_value(json!({ "bytes": "__8=" })).unwrap();
    assert_eq!(test.bytes, &[255, 255]);
    let test: Test = serde_json::from_value(json!({ "bytes": "__8" })).unwrap();
    assert_eq!(test.bytes, &[255, 255]);
}

#[test]
fn incorrect_base64_deserialization() {
    let bogus_value = json!({ "bytes": "not base64!" });
    let err = serde_json::from_value::<Test>(bogus_value).unwrap_err();
    assert!(err.to_string().contains("Invalid byte 32"), "{}", err);
}

#[test]
fn roundtrip_mini_fuzz() {
    const SEED: u64 = 123_456;

    let configs = [
        base64::STANDARD,
        base64::STANDARD_NO_PAD,
        base64::URL_SAFE,
        base64::URL_SAFE_NO_PAD,
    ];

    let mut rng = StdRng::seed_from_u64(SEED);
    for _ in 0..10_000 {
        let len = rng.gen_range(0, 64);
        let mut bytes = vec![0_u8; len];
        rng.fill(&mut bytes[..]);
        let test = Test { bytes };

        let json_string = serde_json::to_string(&test).unwrap();
        let restored: Test = serde_json::from_str(&json_string).unwrap();
        assert_eq!(restored.bytes, test.bytes);

        for &config in &configs {
            let json_string = format!(
                r#"{{ "bytes": "{}" }}"#,
                base64::encode_config(&test.bytes, config)
            );
            let restored: Test = serde_json::from_str(&json_string).unwrap();
            assert_eq!(restored.bytes, test.bytes);
        }
    }
}

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

use bit_vec::BitVec;
use chrono::{DateTime, TimeZone, Utc};
use crypto::{self, Hash, PublicKey};

use std::collections::HashMap;

use super::schema;
use super::ProtobufConvert;
use messages::BinaryForm;

#[test]
fn test_hash_pb_convert() {
    let data = [7; crypto::HASH_SIZE];
    let hash = Hash::from_slice(&data).unwrap();

    let pb_hash = hash.to_pb();
    assert_eq!(&pb_hash.get_data(), &data);

    let hash_round_trip: Hash = ProtobufConvert::from_pb(pb_hash).unwrap();
    assert_eq!(hash_round_trip, hash);
}

#[test]
fn test_hash_wrong_pb_convert() {
    let pb_hash = schema::helpers::Hash::new();
    assert!(<Hash as ProtobufConvert>::from_pb(pb_hash).is_err());

    let mut pb_hash = schema::helpers::Hash::new();
    pb_hash.set_data([7; crypto::HASH_SIZE + 1].to_vec());
    assert!(<Hash as ProtobufConvert>::from_pb(pb_hash).is_err());

    let mut pb_hash = schema::helpers::Hash::new();
    pb_hash.set_data([7; crypto::HASH_SIZE - 1].to_vec());
    assert!(<Hash as ProtobufConvert>::from_pb(pb_hash).is_err());
}

#[test]
fn test_pubkey_pb_convert() {
    let data = [7; crypto::PUBLIC_KEY_LENGTH];
    let key = PublicKey::from_slice(&data).unwrap();

    let pb_key = key.to_pb();
    assert_eq!(&pb_key.get_data(), &data);

    let key_round_trip: PublicKey = ProtobufConvert::from_pb(pb_key).unwrap();
    assert_eq!(key_round_trip, key);
}

#[test]
fn test_pubkey_wrong_pb_convert() {
    let pb_key = schema::helpers::PublicKey::new();
    assert!(<PublicKey as ProtobufConvert>::from_pb(pb_key).is_err());

    let mut pb_key = schema::helpers::PublicKey::new();
    pb_key.set_data([7; crypto::PUBLIC_KEY_LENGTH + 1].to_vec());
    assert!(<PublicKey as ProtobufConvert>::from_pb(pb_key).is_err());

    let mut pb_key = schema::helpers::PublicKey::new();
    pb_key.set_data([7; crypto::PUBLIC_KEY_LENGTH - 1].to_vec());
    assert!(<PublicKey as ProtobufConvert>::from_pb(pb_key).is_err());
}

#[test]
fn test_bitvec_pb_convert() {
    let bv = BitVec::from_bytes(&[0b10100000, 0b00010010]);

    let pb_bv = bv.to_pb();
    let pb_round_trip: BitVec = ProtobufConvert::from_pb(pb_bv).unwrap();
    assert_eq!(pb_round_trip, bv);
}

#[test]
fn test_date_time_pb_convert() {
    let dt = Utc.ymd(2018, 1, 26).and_hms_micro(18, 30, 9, 453_829);
    let pb_dt = dt.to_pb();
    let pb_round_trip: DateTime<Utc> = ProtobufConvert::from_pb(pb_dt).unwrap();
    assert_eq!(pb_round_trip, dt);
}

#[derive(Debug, PartialEq, ProtobufConvert)]
#[exonum(pb = "schema::tests::Point", crate = "crate")]
struct Point {
    x: u32,
    y: u32,
}

#[test]
fn test_simple_struct_round_trip() {
    let point = Point { x: 1, y: 2 };

    let point_pb = point.to_pb();
    let point_convert_round_trip: Point = ProtobufConvert::from_pb(point_pb).unwrap();
    assert_eq!(point_convert_round_trip, point);

    let bytes = point.encode().unwrap();
    let point_encode_round_trip = Point::decode(&bytes).unwrap();
    assert_eq!(point_encode_round_trip, point);
}

#[derive(Debug, PartialEq, ProtobufConvert)]
#[exonum(pb = "schema::tests::TestProtobufConvert", crate = "crate")]
struct StructWithScalarTypes {
    key: PublicKey,
    hash: Hash,
    bit_vec: BitVec,
    time: DateTime<Utc>,
    unsigned_32: u32,
    unsigned_64: u64,
    regular_i32: i32,
    regular_i64: i64,
    fixed_u32: u32,
    fixed_u64: u64,
    fixed_i32: i32,
    fixed_i64: i64,
    float_32: f32,
    float_64: f64,
    boolean: bool,
    s_i32: i32,
    s_i64: i64,
    bytes_field: Vec<u8>,
    string_field: String,
    message_field: Point,
}

#[test]
fn test_scalar_struct_round_trip() {
    let scalar_struct = StructWithScalarTypes {
        key: PublicKey::from_slice(&[8; crypto::PUBLIC_KEY_LENGTH]).unwrap(),
        hash: Hash::from_slice(&[7; crypto::HASH_SIZE]).unwrap(),
        bit_vec: BitVec::from_bytes(&[0b10100000, 0b00010010]),
        time: Utc.ymd(2018, 1, 26).and_hms_micro(18, 30, 9, 453_829),
        unsigned_32: u32::max_value(),
        unsigned_64: u64::max_value(),
        regular_i32: i32::min_value(),
        regular_i64: i64::min_value(),
        fixed_u32: u32::max_value(),
        fixed_u64: u64::max_value(),
        fixed_i32: i32::min_value(),
        fixed_i64: i64::min_value(),
        float_32: std::f32::MAX,
        float_64: std::f64::MAX,
        boolean: true,
        s_i32: i32::min_value(),
        s_i64: i64::min_value(),
        bytes_field: vec![1, 2, 3, 4],
        string_field: "test".to_string(),
        message_field: Point { x: 1, y: 2 },
    };
    let scalar_struct_pb = scalar_struct.to_pb();
    let struct_convert_round_trip: StructWithScalarTypes =
        ProtobufConvert::from_pb(scalar_struct_pb).unwrap();
    assert_eq!(struct_convert_round_trip, scalar_struct);

    let bytes = scalar_struct.encode().unwrap();
    let struct_encode_round_trip = StructWithScalarTypes::decode(&bytes).unwrap();
    assert_eq!(struct_encode_round_trip, scalar_struct);
}

#[derive(Debug, PartialEq, ProtobufConvert)]
#[exonum(pb = "schema::tests::TestProtobufConvertRepeated", crate = "crate")]
struct StructWithRepeatedTypes {
    keys: Vec<PublicKey>,
    bytes_array: Vec<Vec<u8>>,
    string_array: Vec<String>,
    num_array: Vec<u32>,
}

#[test]
fn test_repeated_struct_round_trip() {
    let rep_struct = StructWithRepeatedTypes {
        keys: vec![
            PublicKey::from_slice(&[8; crypto::PUBLIC_KEY_LENGTH]).unwrap(),
            PublicKey::from_slice(&[2; crypto::PUBLIC_KEY_LENGTH]).unwrap(),
        ],
        bytes_array: vec![vec![1, 2, 3], vec![4, 5, 6]],
        string_array: vec![String::from("abc"), String::from("def")],
        num_array: vec![9, 8, 7],
    };
    let rep_struct_pb = rep_struct.to_pb();
    let struct_convert_round_trip: StructWithRepeatedTypes =
        ProtobufConvert::from_pb(rep_struct_pb).unwrap();
    assert_eq!(struct_convert_round_trip, rep_struct);

    let bytes = rep_struct.encode().unwrap();
    let struct_encode_round_trip = StructWithRepeatedTypes::decode(&bytes).unwrap();
    assert_eq!(struct_encode_round_trip, rep_struct);
}

#[derive(Debug, PartialEq, ProtobufConvert)]
#[exonum(pb = "schema::tests::TestProtobufConvertMap", crate = "crate")]
struct StructWithMaps {
    num_map: HashMap<u32, u64>,
    string_map: HashMap<u32, String>,
    bytes_map: HashMap<u32, Vec<u8>>,
    point_map: HashMap<u32, Point>,
    key_string_map: HashMap<String, u64>,
}

#[test]
fn test_struct_with_maps_roundtrip() {
    let map_struct = StructWithMaps {
        num_map: vec![(1, 1), (2, u64::max_value())].into_iter().collect(),
        string_map: vec![(1, String::from("abc")), (2, String::from("def"))]
            .into_iter()
            .collect(),
        bytes_map: vec![(1, vec![1, 2, 3]), (2, vec![3, 4, 5])]
            .into_iter()
            .collect(),
        point_map: vec![(1, Point { x: 1, y: 2 }), (2, Point { x: 3, y: 4 })]
            .into_iter()
            .collect(),
        key_string_map: vec![
            (String::from("abc"), 0),
            (String::from("def"), u64::max_value()),
        ]
        .into_iter()
        .collect(),
    };

    let map_struct_pb = map_struct.to_pb();
    let struct_convert_round_trip: StructWithMaps =
        ProtobufConvert::from_pb(map_struct_pb).unwrap();
    assert_eq!(struct_convert_round_trip, map_struct);

    let bytes = map_struct.encode().unwrap();
    let struct_encode_round_trip = StructWithMaps::decode(&bytes).unwrap();
    assert_eq!(struct_encode_round_trip, map_struct);
}

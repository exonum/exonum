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

use bit_vec::BitVec;
use chrono::{DateTime, TimeZone, Utc};
use exonum_derive::{BinaryValue, ObjectHash};
use exonum_merkledb::BinaryValue;

use std::{borrow::Cow, collections::HashMap};

use super::{schema, ProtobufConvert};
use crate::crypto::{self, Hash, PublicKey};

#[test]
fn test_date_time_pb_convert() {
    let dt = Utc.ymd(2018, 1, 26).and_hms_micro(18, 30, 9, 453_829);
    let pb_dt = dt.to_pb();
    let pb_round_trip: DateTime<Utc> = ProtobufConvert::from_pb(pb_dt).unwrap();
    assert_eq!(pb_round_trip, dt);
}

#[derive(Debug, PartialEq)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "schema::tests::Point")]
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

    let bytes = point.to_bytes();
    let point_encode_round_trip = Point::from_bytes(Cow::from(&bytes)).unwrap();
    assert_eq!(point_encode_round_trip, point);
}

#[derive(Debug, PartialEq)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "schema::tests::TestProtobufConvert")]
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
        bit_vec: BitVec::from_bytes(&[0b_1010_0000, 0b_0001_0010]),
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

    let bytes = scalar_struct.to_bytes();
    let struct_encode_round_trip = StructWithScalarTypes::from_bytes(Cow::from(&bytes)).unwrap();
    assert_eq!(struct_encode_round_trip, scalar_struct);
}

#[protobuf_convert(source = "schema::tests::TestProtobufConvertRepeated")]
#[derive(Debug, PartialEq)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash)]
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

    let bytes = rep_struct.to_bytes();
    let struct_encode_round_trip = StructWithRepeatedTypes::from_bytes(Cow::from(&bytes)).unwrap();
    assert_eq!(struct_encode_round_trip, rep_struct);
}

#[derive(Debug, PartialEq)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "schema::tests::TestProtobufConvertMap")]
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

    let bytes = map_struct.to_bytes();
    let struct_encode_round_trip = StructWithMaps::from_bytes(Cow::from(&bytes)).unwrap();
    assert_eq!(struct_encode_round_trip, map_struct);
}

#[protobuf_convert(source = "schema::tests::TestFixedArrays")]
#[derive(Clone, Copy, Debug, PartialEq)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash)]
struct StructWithFixedArrays {
    fixed_array_8: [u8; 8],
    fixed_array_16: [u8; 16],
    fixed_array_32: [u8; 32],
}

#[test]
#[should_panic(expected = "wrong array size: actual 32, expected 64")]
fn test_fixed_array_pb_convert_invalid_len() {
    let vec = vec![0_u8; 32];
    <[u8; 32]>::from_pb(vec.clone()).unwrap();
    <[u8; 64]>::from_pb(vec).unwrap();
}

#[test]
fn test_struct_with_fixed_arrays_roundtrip() {
    let arr_struct = StructWithFixedArrays {
        fixed_array_8: [1; 8],
        fixed_array_16: [1; 16],
        fixed_array_32: [1; 32],
    };

    let arr_struct_pb = arr_struct.to_pb();
    let struct_convert_round_trip: StructWithFixedArrays =
        ProtobufConvert::from_pb(arr_struct_pb).unwrap();
    assert_eq!(struct_convert_round_trip, arr_struct);

    let bytes = arr_struct.to_bytes();
    let struct_encode_round_trip = StructWithFixedArrays::from_bytes(Cow::from(&bytes)).unwrap();
    assert_eq!(struct_encode_round_trip, arr_struct);
}

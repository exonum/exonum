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

use anyhow as failure; // FIXME: remove once `ProtobufConvert` derive is improved (ECR-4316)
use bit_vec::BitVec;
use chrono::{DateTime, TimeZone, Utc};
use exonum::{
    crypto::{self, Hash, PublicKey},
    merkledb::BinaryValue,
};
use exonum_api::ErrorBody;
use exonum_derive::{BinaryValue, ObjectHash};
use exonum_proto::ProtobufConvert;
use exonum_rust_runtime::{ProtoSourceFile, ProtoSourcesQuery};
use exonum_testkit::{ApiKind, TestKitBuilder};
use pretty_assertions::assert_eq;
use reqwest::{Client, StatusCode};

use std::{borrow::Cow, collections::HashMap};

use crate::{assert_exonum_core_protos, service::Transfer, testkit_with_rust_service};

#[test]
fn test_date_time_pb_convert() {
    let dt = Utc.ymd(2018, 1, 26).and_hms_micro(18, 30, 9, 453_829);
    let pb_dt = dt.to_pb();
    let pb_round_trip: DateTime<Utc> = ProtobufConvert::from_pb(pb_dt).unwrap();
    assert_eq!(pb_round_trip, dt);
}

#[derive(Debug, PartialEq)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "crate::proto::Point")]
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
#[protobuf_convert(source = "crate::proto::TestProtobufConvert")]
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

#[protobuf_convert(source = "crate::proto::TestProtobufConvertRepeated")]
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
#[protobuf_convert(source = "crate::proto::TestProtobufConvertMap")]
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

#[protobuf_convert(source = "crate::proto::TestFixedArrays")]
#[derive(Clone, Copy, Debug, PartialEq)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash)]
struct StructWithFixedArrays {
    fixed_array_8: [u8; 8],
    fixed_array_16: [u8; 16],
    fixed_array_32: [u8; 32],
}

#[test]
fn test_fixed_array_pb_convert_invalid_len() {
    let vec = vec![0_u8; 32];
    <[u8; 32]>::from_pb(vec.clone()).unwrap();
    let err = <[u8; 64]>::from_pb(vec).map(drop).unwrap_err();
    assert!(err
        .to_string()
        .contains("wrong array size: actual 32, expected 64"));
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

#[tokio::test]
async fn core_protos_with_service() {
    let (_, api) = testkit_with_rust_service();
    assert_exonum_core_protos(&api).await;
}

#[tokio::test]
async fn core_protos_without_services() {
    let mut testkit = TestKitBuilder::validator().build();
    assert_exonum_core_protos(&testkit.api()).await;
}

/// Rust-runtime api returns correct source files of the specified artifact.
#[tokio::test]
async fn service_protos_with_service() {
    let (_, api) = testkit_with_rust_service();

    let proto_files: Vec<ProtoSourceFile> = api
        .public(ApiKind::RustRuntime)
        .query(&ProtoSourcesQuery::Artifact {
            name: "test-runtime-api".to_owned(),
            version: "0.0.1".parse().unwrap(),
        })
        .get("proto-sources")
        .await
        .expect("Rust runtime Api unexpectedly failed");

    const EXPECTED_CONTENT: &str = include_str!("proto/service.proto");

    assert_eq!(proto_files.len(), 1);
    assert_eq!(proto_files[0].name, "service.proto".to_string());
    assert_eq!(proto_files[0].content, EXPECTED_CONTENT.to_string());
}

/// Rust-runtime API should return error in case of an incorrect artifact.
#[tokio::test]
async fn service_protos_with_incorrect_service() {
    use exonum::runtime::{ArtifactId, RuntimeIdentifier};

    let (_, api) = testkit_with_rust_service();

    let artifact_id = ArtifactId::new(
        RuntimeIdentifier::Rust,
        "invalid-service",
        "0.0.1".parse().unwrap(),
    )
    .unwrap();
    let artifact_query = ProtoSourcesQuery::Artifact {
        name: artifact_id.name.clone(),
        version: artifact_id.version.clone(),
    };
    let error = api
        .public(ApiKind::RustRuntime)
        .query(&artifact_query)
        .get::<Vec<ProtoSourceFile>>("proto-sources")
        .await
        .expect_err("Rust runtime Api returns a fake source!");

    assert_eq!(&error.body.title, "Artifact sources not found");
    assert_eq!(
        error.body.detail,
        format!("Unable to find sources for artifact {}", artifact_id)
    );
}

#[tokio::test]
async fn request_to_pb_endpoint() -> anyhow::Result<()> {
    let (_, api) = testkit_with_rust_service();
    let transfer = Transfer {
        message: "test".to_owned(),
        seed: 42,
    };

    // Send `Transfer` using standard JSON encoding.
    let response: Transfer = api
        .public(ApiKind::Service("test-runtime-api"))
        .query(&transfer)
        .post("transfer")
        .await?;
    assert_eq!(response, transfer);

    // Send `Transfer` using Protobuf encoding manually.
    let proto_bytes = transfer.to_bytes();
    let url = api.public_url("api/services/test-runtime-api/transfer");
    let response: Transfer = Client::new()
        .post(&url)
        .header("Content-Type", "application/octet-stream")
        .body(proto_bytes)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    assert_eq!(response, transfer);

    // Send `Transfer` using testkit API.
    let response: Transfer = api
        .public(ApiKind::Service("test-runtime-api"))
        .query(&transfer)
        .post_pb("transfer")
        .await?;
    assert_eq!(response, transfer);

    // Attempt to send `Transfer` using invalid encoding.
    let err_response = Client::new()
        .post(&url)
        .header("Content-Type", "application/octet-stream")
        .body(b"Not valid Protobuf!".to_vec())
        .send()
        .await?;
    assert_eq!(err_response.status(), StatusCode::BAD_REQUEST);
    let err_response: ErrorBody = err_response.json().await?;
    assert_eq!(err_response.title, "Cannot parse Protobuf message");

    Ok(())
}

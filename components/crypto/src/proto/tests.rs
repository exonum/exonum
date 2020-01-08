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

use super::{schema::types, ProtobufConvert};
use crate::{Hash, PublicKey, Signature, HASH_SIZE, PUBLIC_KEY_LENGTH, SIGNATURE_LENGTH};

#[test]
fn test_hash_pb_convert() {
    let data = [7; HASH_SIZE];
    let hash = Hash::from_slice(&data).unwrap();

    let pb_hash = hash.to_pb();
    assert_eq!(&pb_hash.get_data(), &data);

    let hash_round_trip: Hash = ProtobufConvert::from_pb(pb_hash).unwrap();
    assert_eq!(hash_round_trip, hash);
}

#[test]
fn test_hash_wrong_pb_convert() {
    let pb_hash = types::Hash::new();
    assert!(<Hash as ProtobufConvert>::from_pb(pb_hash).is_err());

    let mut pb_hash = types::Hash::new();
    pb_hash.set_data([7; HASH_SIZE + 1].to_vec());
    assert!(<Hash as ProtobufConvert>::from_pb(pb_hash).is_err());

    let mut pb_hash = types::Hash::new();
    pb_hash.set_data([7; HASH_SIZE - 1].to_vec());
    assert!(<Hash as ProtobufConvert>::from_pb(pb_hash).is_err());
}

#[test]
fn test_pubkey_pb_convert() {
    let data = [7; PUBLIC_KEY_LENGTH];
    let key = PublicKey::from_slice(&data).unwrap();

    let pb_key = key.to_pb();
    assert_eq!(&pb_key.get_data(), &data);

    let key_round_trip: PublicKey = ProtobufConvert::from_pb(pb_key).unwrap();
    assert_eq!(key_round_trip, key);
}

#[test]
fn test_pubkey_wrong_pb_convert() {
    let pb_key = types::PublicKey::new();
    assert!(<PublicKey as ProtobufConvert>::from_pb(pb_key).is_err());

    let mut pb_key = types::PublicKey::new();
    pb_key.set_data([7; PUBLIC_KEY_LENGTH + 1].to_vec());
    assert!(<PublicKey as ProtobufConvert>::from_pb(pb_key).is_err());

    let mut pb_key = types::PublicKey::new();
    pb_key.set_data([7; PUBLIC_KEY_LENGTH - 1].to_vec());
    assert!(<PublicKey as ProtobufConvert>::from_pb(pb_key).is_err());
}

#[test]
fn test_signature_pb_convert() {
    let data: &[u8] = &[8; SIGNATURE_LENGTH];
    let sign = Signature::from_slice(data).unwrap();

    let pb_sign = sign.to_pb();
    assert_eq!(pb_sign.get_data(), data);

    let sign_round_trip: Signature = ProtobufConvert::from_pb(pb_sign).unwrap();
    assert_eq!(sign_round_trip, sign);
}

#[test]
fn test_signature_wrong_pb_convert() {
    let pb_sign = types::Signature::new();
    assert!(<Signature as ProtobufConvert>::from_pb(pb_sign).is_err());

    let mut pb_sign = types::Signature::new();
    pb_sign.set_data([8; SIGNATURE_LENGTH + 1].to_vec());
    assert!(<Signature as ProtobufConvert>::from_pb(pb_sign).is_err());

    let mut pb_sign = types::Signature::new();
    pb_sign.set_data([8; SIGNATURE_LENGTH - 1].to_vec());
    assert!(<Signature as ProtobufConvert>::from_pb(pb_sign).is_err());
}

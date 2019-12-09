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

use crate::proto;
use exonum_crypto::Hash;
use exonum_merkledb::{BinaryValue, ObjectHash};
use exonum_proto::ProtobufConvert;
use protobuf::Message;

use std::{borrow::Cow, collections::BTreeMap};

/// Protobuf wrapper type to store small maps of non-scalar keys and values.
#[derive(Debug, Default, Clone, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct BinaryMap<K: Ord, V>(pub BTreeMap<K, V>);

#[derive(ProtobufConvert)]
#[protobuf_convert(source = "proto::schema::binary_map::KeyValue")]
struct KeyValue {
    key: Vec<u8>,
    value: Vec<u8>,
}

fn pair_to_key_value_pb<K, V>(pair: (&K, &V)) -> crate::proto::schema::binary_map::KeyValue
where
    K: BinaryValue,
    V: BinaryValue,
{
    KeyValue {
        key: pair.0.to_bytes(),
        value: pair.1.to_bytes(),
    }
    .to_pb()
}

fn key_value_pb_to_pair<K, V>(
    pb: crate::proto::schema::binary_map::KeyValue,
) -> Result<(K, V), failure::Error>
where
    K: BinaryValue,
    V: BinaryValue,
{
    let KeyValue { key, value } = KeyValue::from_pb(pb)?;
    let key = K::from_bytes(key.into())?;
    let value = V::from_bytes(value.into())?;
    Ok((key, value))
}

impl<K, V> ProtobufConvert for BinaryMap<K, V>
where
    K: BinaryValue + Ord,
    V: BinaryValue,
{
    type ProtoStruct = crate::proto::schema::binary_map::BinaryMap;

    fn to_pb(&self) -> Self::ProtoStruct {
        let mut proto_struct = Self::ProtoStruct::new();
        proto_struct.inner = self
            .0
            .iter()
            .map(pair_to_key_value_pb)
            .collect::<Vec<_>>()
            .into();
        proto_struct
    }

    fn from_pb(proto_struct: Self::ProtoStruct) -> Result<Self, failure::Error> {
        let inner = proto_struct
            .inner
            .into_iter()
            .map(key_value_pb_to_pair)
            .collect::<Result<_, failure::Error>>()?;
        Ok(Self(inner))
    }
}

impl<K, V> BinaryValue for BinaryMap<K, V>
where
    K: BinaryValue + Ord,
    V: BinaryValue,
{
    fn to_bytes(&self) -> Vec<u8> {
        self.to_pb()
            .write_to_bytes()
            .expect("Error while serializing value")
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Result<Self, failure::Error> {
        let mut pb = <Self as ProtobufConvert>::ProtoStruct::new();
        pb.merge_from_bytes(bytes.as_ref())?;
        Self::from_pb(pb)
    }
}

impl<K, V> ObjectHash for BinaryMap<K, V>
where
    K: BinaryValue + Ord,
    V: BinaryValue,
{
    fn object_hash(&self) -> Hash {
        exonum_crypto::hash(&self.to_bytes())
    }
}

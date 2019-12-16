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

use std::{borrow::Cow, collections::BTreeMap, iter::FromIterator};

/// Protobuf wrapper type to store small maps of non-scalar keys and values.
/// Stored keys are ordered and duplicate keys are forbidden.
#[derive(Debug, Default, Clone, Eq, PartialEq, Ord, PartialOrd)]
#[derive(Serialize, Deserialize)]
pub struct OrderedMap<K: Ord, V>(pub BTreeMap<K, V>);

#[derive(ProtobufConvert)]
#[protobuf_convert(source = "proto::schema::ordered_map::KeyValue")]
struct KeyValue {
    key: String,
    value: Vec<u8>,
}

fn pair_to_key_value_pb<K, V>(
    pair: (&K, &V),
) -> Result<crate::proto::schema::ordered_map::KeyValue, failure::Error>
where
    K: BinaryValue,
    V: BinaryValue,
{
    Ok(KeyValue {
        key: String::from_utf8(pair.0.to_bytes())?,
        value: pair.1.to_bytes(),
    }
    .to_pb())
}

fn key_value_pb_to_pair<K, V>(
    pb: crate::proto::schema::ordered_map::KeyValue,
) -> Result<(K, V), failure::Error>
where
    K: BinaryValue,
    V: BinaryValue,
{
    let KeyValue { key, value } = KeyValue::from_pb(pb)?;
    let key = K::from_bytes(Cow::Borrowed(key.as_bytes()))?;
    let value = V::from_bytes(value.into())?;
    Ok((key, value))
}

impl<K, V> ProtobufConvert for OrderedMap<K, V>
where
    K: BinaryValue + Ord,
    V: BinaryValue,
{
    type ProtoStruct = crate::proto::schema::ordered_map::OrderedMap;

    fn to_pb(&self) -> Self::ProtoStruct {
        let mut proto_struct = Self::ProtoStruct::new();
        proto_struct.entry = self
            .0
            .iter()
            .map(pair_to_key_value_pb)
            .collect::<Result<Vec<_>, failure::Error>>()
            .expect("Map contains invalid utf-8 keys")
            .into();
        proto_struct
    }

    fn from_pb(proto_struct: Self::ProtoStruct) -> Result<Self, failure::Error> {
        let values = proto_struct
            .entry
            .into_iter()
            .map(key_value_pb_to_pair)
            .collect::<Result<Vec<(K, V)>, failure::Error>>()?;

        let check_key_ordering = |k: &[(K, V)]| {
            let (prev_key, key) = (&k[0].0, &k[1].0);
            prev_key < key
        };

        ensure!(
            values.windows(2).all(check_key_ordering),
            "Invalid keys ordering or duplicate keys found in BinaryMap"
        );

        Ok(Self(BTreeMap::from_iter(values.into_iter())))
    }
}

//TODO: Add generic support to BinaryValue derive macro [ECR-3955].
impl<K, V> BinaryValue for OrderedMap<K, V>
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

//TODO: Add generic support to ObjectHash derive macro [ECR-3955].
impl<K, V> ObjectHash for OrderedMap<K, V>
where
    K: BinaryValue + Ord,
    V: BinaryValue,
{
    fn object_hash(&self) -> Hash {
        exonum_crypto::hash(&self.to_bytes())
    }
}

#[cfg(test)]
mod tests {
    use protobuf::RepeatedField;

    use crate::proto::{
        schema::ordered_map::{KeyValue, OrderedMap as PbOrderedMap},
        OrderedMap,
    };
    use exonum_proto::ProtobufConvert;

    #[test]
    #[should_panic(expected = "Map contains invalid utf-8 key")]
    fn non_utf8_keys() {
        let mut map = OrderedMap::default();
        map.0.insert(vec![192, 128], vec![10]);

        let pb_map = map.to_pb();
        let _de_map: OrderedMap<Vec<u8>, Vec<u8>> = ProtobufConvert::from_pb(pb_map).unwrap();
    }

    #[test]
    fn unordered_keys() {
        let mut kv = KeyValue::new();
        kv.set_key("bbb".to_owned());

        let mut kv2 = KeyValue::new();
        kv2.set_key("aaa".to_owned());

        // Unordered keys.
        let mut map = PbOrderedMap::new();
        map.set_entry(RepeatedField::from_vec(vec![kv.clone(), kv2.clone()]));

        let res: Result<OrderedMap<String, Vec<u8>>, failure::Error> =
            ProtobufConvert::from_pb(map);
        res.unwrap_err()
            .to_string()
            .contains("Invalid keys ordering");

        // Duplicate keys.
        let mut map = PbOrderedMap::new();
        map.set_entry(RepeatedField::from_vec(vec![kv2.clone(), kv, kv2]));

        let res: Result<OrderedMap<String, Vec<u8>>, failure::Error> =
            ProtobufConvert::from_pb(map);
        res.unwrap_err()
            .to_string()
            .contains("Invalid keys ordering");
    }
}

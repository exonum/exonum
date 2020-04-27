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

use anyhow::ensure;
use exonum_crypto::Hash;
use exonum_merkledb::{BinaryValue, ObjectHash};
use exonum_proto::ProtobufConvert;
use protobuf::Message;

use std::{borrow::Cow, collections::BTreeMap, iter::FromIterator};

use crate::proto;

/// Protobuf-encodable type to store small maps of non-scalar keys and values.
///
/// This structure uses on `KeyValueSequence` from `key_value_sequence.proto` as
/// a backend, but adds the verification logic to it:
///
/// - Keys are sorted in a lexicographical order;
/// - Duplicate keys are forbidden.
#[derive(Debug, Default, Clone, Eq, PartialEq, Ord, PartialOrd)]
#[derive(Serialize, Deserialize)]
pub(crate) struct OrderedMap<K: Ord, V>(pub BTreeMap<K, V>);

#[derive(ProtobufConvert)]
#[protobuf_convert(source = "proto::schema::key_value_sequence::KeyValue")]
struct KeyValue {
    key: String,
    value: Vec<u8>,
}

fn pair_to_key_value_pb<K, V>(
    pair: (&K, &V),
) -> anyhow::Result<crate::proto::schema::key_value_sequence::KeyValue>
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
    pb: crate::proto::schema::key_value_sequence::KeyValue,
) -> anyhow::Result<(K, V)>
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
    type ProtoStruct = crate::proto::schema::key_value_sequence::KeyValueSequence;

    fn to_pb(&self) -> Self::ProtoStruct {
        let mut proto_struct = Self::ProtoStruct::new();
        proto_struct.entries = self
            .0
            .iter()
            .map(pair_to_key_value_pb)
            .collect::<anyhow::Result<Vec<_>>>()
            .expect("Map contains invalid utf-8 keys")
            .into();
        proto_struct
    }

    fn from_pb(proto_struct: Self::ProtoStruct) -> anyhow::Result<Self> {
        let values = proto_struct
            .entries
            .into_iter()
            .map(key_value_pb_to_pair)
            .collect::<anyhow::Result<Vec<_>>>()?;

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

    fn from_bytes(bytes: Cow<'_, [u8]>) -> anyhow::Result<Self> {
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
    use exonum_proto::ProtobufConvert;
    use protobuf::RepeatedField;

    use super::OrderedMap;
    use crate::proto::schema::key_value_sequence::{
        KeyValue, KeyValueSequence as PbKeyValueSequence,
    };

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
        let mut map = PbKeyValueSequence::new();
        map.set_entries(RepeatedField::from_vec(vec![kv.clone(), kv2.clone()]));

        let res = OrderedMap::<String, Vec<u8>>::from_pb(map);
        res.unwrap_err()
            .to_string()
            .contains("Invalid keys ordering");

        // Duplicate keys.
        let mut map = PbKeyValueSequence::new();
        map.set_entries(RepeatedField::from_vec(vec![kv2.clone(), kv, kv2]));

        let res = OrderedMap::<String, Vec<u8>>::from_pb(map);
        res.unwrap_err()
            .to_string()
            .contains("Invalid keys ordering");
    }
}

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

//! Module of the rust-protobuf generated files.

// For protobuf generated files.
#![allow(bare_trait_objects)]

use protobuf::{RepeatedField, well_known_types::Empty};
use failure::Error;
use std::{iter::FromIterator, borrow::Cow};

use exonum_crypto::proto::*;
use exonum_proto::ProtobufConvert;
use crate::{BinaryKey, BinaryValue, proof_map_index::ProofPath};

pub use self::proof::*;

include!(concat!(env!("OUT_DIR"), "/protobuf_mod.rs"));

impl<K, V> ProtobufConvert for crate::MapProof<K, V>
    where
        K: BinaryKey,
        V: BinaryValue,
        Vec<(K, Option<V>)>: FromIterator<(<K as ToOwned>::Owned, Option<V>)>,
{
    type ProtoStruct = MapProof;

    fn to_pb(&self) -> Self::ProtoStruct {
        let mut map_proof = MapProof::new();

        let proof: Vec<MapProofEntry> = self
            .proof_unchecked()
            .iter()
            .map(|(p, h)| {
                let mut entry = MapProofEntry::new();
                entry.set_hash(h.to_pb());
                entry.set_proof_path(p.as_bytes().to_vec());
                entry
            })
            .collect();

        let entries: Vec<OptionalEntry> = self
            .all_entries_unchecked()
            .map(|(key, value)| {
                let mut entry = OptionalEntry::new();
                let mut buf = vec![0u8; key.size()];
                key.write(&mut buf);
                entry.set_key(buf.to_vec());

                match value {
                    Some(value) => entry.set_value(value.to_bytes()),
                    None => entry.set_no_value(Empty::new()),
                }

                entry
            })
            .collect();

        map_proof.set_proof(RepeatedField::from_vec(proof));
        map_proof.set_entries(RepeatedField::from_vec(entries));

        map_proof
    }

    fn from_pb(pb: Self::ProtoStruct) -> Result<Self, Error> {
        let proof = pb
            .get_proof()
            .iter()
            .map(|entry| {
                Ok((
                    ProofPath::read(entry.get_proof_path()),
                    exonum_crypto::Hash::from_pb(entry.get_hash().clone())?,
                ))
            })
            .collect::<Result<Vec<_>, Error>>()?;

        let entries = pb
            .get_entries()
            .iter()
            .map(|entry| {
                let key = K::read(entry.get_key());

                let value = if entry.has_value() {
                    Some(V::from_bytes(Cow::Borrowed(entry.get_value()))?)
                } else {
                    None
                };

                Ok((key, value))
            })
            .collect::<Result<Vec<_>, Error>>()?;

        let mut map_proof = crate::MapProof::new().add_proof_entries(proof);

        for entry in entries {
            map_proof = match entry.1 {
                Some(value) => map_proof.add_entry(entry.0, value),
                None => map_proof.add_missing(entry.0),
            };
        };

        Ok(map_proof)
    }
}

#[cfg(test)]
mod tests {
    use std::fmt;
    use exonum_proto::ProtobufConvert;

    use crate::{TemporaryDB, ProofMapIndex,Database, BinaryKey, BinaryValue, ObjectHash, MapProof};

    #[test]
    fn serialize_map_proof() {
        let db = TemporaryDB::default();
        let storage = db.fork();

        let mut table = ProofMapIndex::new("index", &storage);

        let proof = table.get_proof(0);
        assert_proof_roundtrip(proof);

        for i in 0..10 {
            table.put(&i, i);
        }

        let proof = table.get_proof(5);
        assert_proof_roundtrip(proof);

        let proof = table.get_multiproof(5..15);
        assert_proof_roundtrip(proof);
    }

    fn assert_proof_roundtrip<K, V>(proof: MapProof<K, V>)
        where
            K: BinaryKey + ObjectHash + fmt::Debug,
            V: BinaryValue + ObjectHash + fmt::Debug,
            MapProof<K, V>: ProtobufConvert + PartialEq,
    {
        let pb = proof.to_pb();
        let deserialized: MapProof<K, V> = MapProof::from_pb(pb).unwrap();
        let checked_proof = deserialized
            .check()
            .expect("deserialized proof is not valid");

        assert_eq!(proof, deserialized);
        assert_eq!(
            checked_proof.index_hash(),
            proof.check().unwrap().index_hash()
        );
    }
}
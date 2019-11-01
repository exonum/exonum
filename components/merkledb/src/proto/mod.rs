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

use failure::Error;
use protobuf::{well_known_types::Empty, RepeatedField};
use std::borrow::Cow;

use crate::{proof_map_index::ProofPath, BinaryKey, BinaryValue};
use exonum_crypto::proto::*;
use exonum_proto::ProtobufConvert;

pub use self::{list_proof::*, map_proof::*};

include!(concat!(env!("OUT_DIR"), "/protobuf_mod.rs"));

impl<K, V> ProtobufConvert for crate::MapProof<K, V>
where
    K: BinaryKey + ToOwned<Owned = K>,
    V: BinaryValue,
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
                let mut buf = vec![0_u8; key.size()];
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
                    ensure!(
                        entry.has_no_value(),
                        "malformed message, no_value is absent"
                    );
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
        }

        Ok(map_proof)
    }
}

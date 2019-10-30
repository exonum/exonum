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

pub use self::list_proof::*;
use exonum_crypto::proto::*;

include!(concat!(env!("OUT_DIR"), "/protobuf_mod.rs"));

#[cfg(test)]
mod tests {
    use crate::{BinaryValue, Database, ListProof, ObjectHash, ProofListIndex, TemporaryDB};
    use exonum_proto::ProtobufConvert;

    #[test]
    fn serialize_list_proof() {
        let db = TemporaryDB::default();
        let storage = db.fork();

        let mut table = ProofListIndex::new("index", &storage);

        let proof = table.get_proof(0);
        assert_list_proof_roundtrip(&proof);

        for i in 0..256 {
            table.push(i);
        }

        let proof = table.get_proof(5);
        assert_list_proof_roundtrip(&proof);

        let proof = table.get_range_proof(250..260);
        assert_list_proof_roundtrip(&proof);
    }

    fn assert_list_proof_roundtrip<V>(proof: &ListProof<V>)
    where
        V: BinaryValue + ObjectHash + std::fmt::Debug,
        ListProof<V>: ProtobufConvert + PartialEq,
    {
        let pb = proof.to_pb();
        let deserialized: ListProof<V> = ListProof::from_pb(pb).unwrap();
        let checked_proof = deserialized
            .check()
            .expect("deserialized proof is not valid");

        assert_eq!(proof, &deserialized);
        assert_eq!(
            checked_proof.index_hash(),
            proof.check().unwrap().index_hash()
        );
    }
}

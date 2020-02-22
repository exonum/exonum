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

use exonum_crypto::{Hash, PublicKey};
use exonum_derive::{BinaryValue, ObjectHash};
use exonum_merkledb::{
    proof_map::MapProofError, BinaryValue, MapProof, ObjectHash, ValidationError,
};
use exonum_proto::ProtobufConvert;
use failure::Fail;

use std::borrow::Cow;

use crate::{
    blockchain::CallInBlock,
    helpers::{byzantine_quorum, Height, OrderedMap, ValidatorId},
    messages::{Precommit, Verified},
    proto::schema,
    runtime::ExecutionError,
};

/// Trait that represents a key in block header entry map. Provides
/// a mapping between `NAME` of the entry and its value.
///
/// # Examples
///
/// See [`Block::get_entry()`].
///
/// [`Block::get_entry()`]: struct.Block.html#method.get_entry
pub trait BlockHeaderKey {
    /// Key name.
    const NAME: &'static str;
    /// Type of the value associated with this key.
    type Value: BinaryValue;
}

/// Identifier of a proposer of the block.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct ProposerId(());

impl BlockHeaderKey for ProposerId {
    const NAME: &'static str = "proposer_id";
    type Value = ValidatorId;
}

/// Expandable set of headers allowed to be added to the block.
///
/// In a serialized form, headers are represented as a sequence of
/// pairs, in which first element is a string (header name), and the
/// second element is a byte sequence (deserialization format for which
/// depends on the header name).
#[derive(Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[derive(Serialize, Deserialize)]
#[derive(ProtobufConvert)]
#[protobuf_convert(source = "schema::blockchain::AdditionalHeaders")]
pub struct AdditionalHeaders {
    /// Underlying storage for additional headers.
    headers: OrderedMap<String, Vec<u8>>,
}

impl AdditionalHeaders {
    /// New instance of `AdditionalHeaders`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert new header to the map.
    pub fn insert<K: BlockHeaderKey>(&mut self, value: K::Value) {
        self.headers.0.insert(K::NAME.into(), value.into_bytes());
    }

    /// Get header from the map.
    pub fn get<K: BlockHeaderKey>(&self) -> Option<&[u8]> {
        self.headers.0.get(K::NAME).map(Vec::as_slice)
    }
}

/// Header of a block.
///
/// A block is essentially a list of transactions. Blocks are produced as
/// a result of the consensus algorithm (thus authenticated by the supermajority of validators)
/// and are applied atomically to the blockchain state. The header contains a block summary,
/// such as the number of transactions and the transactions root hash, but not
/// the transactions themselves.
///
/// Note that this structure is export-only, meaning that one can rely on the serialization format
/// provided by corresponding Protobuf definitions, but cannot expect Exonum nodes
/// or the `exonum` crate to accept and process `Block`s created externally.
#[derive(Clone, PartialEq, Eq, Ord, PartialOrd, Debug)]
#[derive(Serialize, Deserialize)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "schema::blockchain::Block")]
pub struct Block {
    /// Height of the block, which is also the number of this particular
    /// block in the blockchain.
    pub height: Height,
    /// Number of transactions in this block.
    pub tx_count: u32,
    /// Hash link to the previous block in the blockchain.
    pub prev_hash: Hash,
    /// Root hash of the Merkle tree of transactions in this block.
    pub tx_hash: Hash,
    /// Hash of the blockchain state after applying transactions in the block.
    pub state_hash: Hash,
    /// Root hash of the Merkle Patricia tree of the erroneous calls performed within the block.
    /// These calls can include transactions, `before_transactions` and/or `after_transactions` hooks
    /// for services.
    pub error_hash: Hash,
    /// Additional information that can be added into the block.
    pub additional_headers: AdditionalHeaders,
}

impl Block {
    /// Inserts new additional header to the block.
    #[doc(hidden)]
    pub fn add_header<K: BlockHeaderKey>(&mut self, value: K::Value) {
        self.additional_headers.insert::<K>(value);
    }

    /// Gets the value of an additional header for the specified key type, which is specified via
    /// the type parameter.
    ///
    /// # Examples
    ///
    /// ```
    /// # use exonum::crypto::Hash;
    /// # use exonum::blockchain::{Block, BlockHeaderKey, AdditionalHeaders};
    /// # use exonum::helpers::Height;
    /// # use exonum::merkledb::BinaryValue;
    /// # use failure::Error;
    /// # use std::borrow::Cow;
    /// // Suppose we store a list of active service IDs in a block.
    /// // We can do this by defining a corresponding BlockHeaderKey implementation.
    /// struct ActiveServices {
    ///     service_ids: Vec<u32>,
    /// }
    ///
    /// # impl BinaryValue for ActiveServices {
    /// #     fn to_bytes(&self) -> Vec<u8> { vec![] }
    /// #     fn from_bytes(bytes: Cow<'_, [u8]>) -> Result<Self, Error> {
    /// #         Ok(Self { service_ids: vec![] })
    /// #     }
    /// # }
    /// // To implement `BlockHeaderKey` we need to provide the key name and a corresponding
    /// // value type. In this case it's `Self`.
    /// impl BlockHeaderKey for ActiveServices {
    ///     const NAME: &'static str = "active_services";
    ///     type Value = Self;
    /// }
    ///
    /// // Create an empty block.
    /// let block = Block {
    ///     # height: Height(0),
    ///     # tx_count: 0,
    ///     # prev_hash: Hash::zero(),
    ///     # tx_hash: Hash::zero(),
    ///     # state_hash: Hash::zero(),
    ///     # error_hash: Hash::zero(),
    ///     // other fields skipped...
    ///     additional_headers: AdditionalHeaders::new(),
    /// };
    ///
    /// let services = block.get_header::<ActiveServices>().unwrap();
    /// assert!(services.is_none());
    /// ```
    pub fn get_header<K: BlockHeaderKey>(&self) -> Result<Option<K::Value>, failure::Error> {
        self.additional_headers
            .get::<K>()
            .map(|bytes: &[u8]| K::Value::from_bytes(Cow::Borrowed(bytes)))
            .transpose()
    }
}

/// Block with its `Precommit` messages.
///
/// This structure contains enough information to prove the correctness of
/// a block. It consists of the block itself and the `Precommit`
/// messages related to this block.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ProtobufConvert)]
#[protobuf_convert(source = "schema::proofs::BlockProof")]
pub struct BlockProof {
    /// Block header containing such information as the ID of the node which
    /// proposed the block, the height of the block, the number of transactions
    /// in the block, etc.
    pub block: Block,
    /// List of `Precommit` messages for the block.
    pub precommits: Vec<Verified<Precommit>>,

    /// No-op field for forward compatibility.
    #[protobuf_convert(skip)]
    #[serde(default, skip)]
    non_exhaustive: (),
}

impl BlockProof {
    /// Creates a new `BlockProof` object.
    pub fn new(block: Block, precommits: Vec<Verified<Precommit>>) -> Self {
        Self {
            block,
            precommits,
            non_exhaustive: (),
        }
    }

    /// Verifies that the block in this proof is endorsed by the Byzantine majority of provided
    /// validators.
    pub fn verify(&self, validator_keys: &[PublicKey]) -> Result<(), ProofError> {
        if self.precommits.len() < byzantine_quorum(validator_keys.len()) {
            return Err(ProofError::NoQuorum);
        }
        if self.precommits.len() > validator_keys.len() {
            return Err(ProofError::DoubleEndorsement);
        }

        let correct_heights = self
            .precommits
            .iter()
            .all(|precommit| precommit.payload().height == self.block.height);
        if !correct_heights {
            return Err(ProofError::IncorrectHeight);
        }

        let block_hash = self.block.object_hash();
        let correct_block_hashes = self
            .precommits
            .iter()
            .all(|precommit| precommit.payload().block_hash == block_hash);
        if !correct_block_hashes {
            return Err(ProofError::IncorrectBlockHash);
        }

        let mut endorsements = vec![false; validator_keys.len()];
        for precommit in &self.precommits {
            let validator_id = precommit.payload().validator.0 as usize;
            let expected_key = *validator_keys
                .get(validator_id)
                .ok_or(ProofError::IncorrectValidatorId)?;
            if expected_key != precommit.author() {
                return Err(ProofError::ValidatorKeyMismatch);
            }
            if endorsements[validator_id] {
                return Err(ProofError::DoubleEndorsement);
            }
            endorsements[validator_id] = true;
        }

        // This assertion should always hold. Indeed, we've checked that there are +2/3 precommits
        // and that there are no double endorsements; hence, the block should be approved
        // by +2/3 validators.
        debug_assert!(
            endorsements.iter().filter(|&&flag| flag).count()
                >= byzantine_quorum(validator_keys.len())
        );

        Ok(())
    }
}

/// Errors that can occur during verification of `BlockProof`s, `IndexProof`s and `CallProof`s.
#[derive(Debug, Fail)]
pub enum ProofError {
    /// The block is authorized by an insufficient number of precommits.
    #[fail(display = "Insufficient number of precommits")]
    NoQuorum,

    /// Block height mentioned in at least one of precommits differs from the height mentioned
    /// in the block header.
    #[fail(display = "Incorrect block height in at least one of precommits")]
    IncorrectHeight,

    /// Hash of the block in at least one precommit differs from that of the real block.
    #[fail(display = "Incorrect block hash in at least one of precommits")]
    IncorrectBlockHash,

    /// Validator ID mentioned in at least one precommit is incorrect.
    #[fail(display = "Incorrect validator ID in at least one of precommits")]
    IncorrectValidatorId,

    /// Key of a validator differs from the expected.
    #[fail(
        display = "Mismatch between key in precommit message and key of corresponding validator"
    )]
    ValidatorKeyMismatch,

    /// The same validator has authorized several precommits.
    #[fail(display = "Multiple precommits from the same validator")]
    DoubleEndorsement,

    /// The proof does not actually prove existence of any index.
    #[fail(display = "proof does not actually prove existence of any entry")]
    NoEntry,

    /// The proof purports to prove existence of more than one index.
    #[fail(display = "proof purports to prove existence of more than one entry")]
    AmbiguousEntry,

    /// Entry proof is incorrect.
    #[fail(display = "entry proof is incorrect: {}", _0)]
    IncorrectEntryProof(#[fail(cause)] ValidationError<MapProofError>),

    /// Call status embedded in the proof is malformed; it could not be generated by a
    /// properly functioning Exonum node.
    #[fail(display = "call status embedded in the proof is malformed")]
    MalformedStatus,

    /// Never actually generated.
    #[doc(hidden)]
    #[fail(display = "")]
    __NonExhaustive,
}

/// Proof of authenticity for a single index within the database.
#[derive(Debug, Clone, Serialize, Deserialize, ProtobufConvert)]
#[protobuf_convert(source = "schema::proofs::IndexProof")]
pub struct IndexProof {
    /// Proof of authenticity for the block header.
    #[serde(flatten)]
    pub block_proof: BlockProof,

    /// Proof of authenticity for the index. Must contain a single key - a full index name
    /// in the form `$service_name.$name_within_service`, e.g., `cryptocurrency.wallets`.
    /// The root hash of the proof must be equal to the `state_hash` mentioned in `block_proof`.
    pub index_proof: MapProof<String, Hash>,

    /// No-op field for forward compatibility.
    #[protobuf_convert(skip)]
    #[serde(default, skip)]
    non_exhaustive: (),
}

impl IndexProof {
    /// Creates a new `IndexProof` object.
    pub fn new(block_proof: BlockProof, index_proof: MapProof<String, Hash>) -> Self {
        Self {
            block_proof,
            index_proof,
            non_exhaustive: (),
        }
    }

    /// Verifies this proof, returning the full index name (e.g., `cryptocurrency.wallets`)
    /// and its hash on success.
    pub fn verify(&self, validator_keys: &[PublicKey]) -> Result<(&str, Hash), ProofError> {
        self.block_proof.verify(validator_keys)?;

        // The index proof should feature exactly one present entry.
        let mut unchecked_entries = self.index_proof.all_entries_unchecked();
        let (name, maybe_hash) = unchecked_entries.next().ok_or(ProofError::NoEntry)?;
        if unchecked_entries.next().is_some() {
            return Err(ProofError::AmbiguousEntry);
        }
        let index_hash = *maybe_hash.ok_or(ProofError::NoEntry)?;
        self.index_proof
            .check_against_hash(self.block_proof.block.state_hash)
            .map_err(ProofError::IncorrectEntryProof)?;
        Ok((name.as_str(), index_hash))
    }
}

/// Proof of authenticity for a single top-level call in a block, such as a [transaction].
///
/// The proof consists of two parts:
///
/// - `block_proof`: block header with the proof of authenticity
/// - `call_proof`: proof from the error aggregator (i.e., a `ProofMapIndex` the Merkle root
///   of which is recorded in the block header as `error_hash`).
///
/// For an execution that resulted in an error, `call_proof` will be a proof of existence.
/// If a transaction was executed successfully, `call_proof` will be a proof of absence.
/// Since the number of transactions in a block is mentioned in the block header, the user
/// will be able to distinguish absence of error (meaning successful execution) from
/// the absence of a transaction with such an index. Indeed, if the index is less
/// than amount of transactions in block, the proof denotes successful execution;
/// otherwise, the transaction with the given index does not exist in the block.
///
/// Similarly, execution errors of the `before_transactions` / `after_transactions`
/// hooks can be proven to external clients. Discerning successful execution
/// from a non-existing service requires prior knowledge though.
///
/// `CallProof`s should not be mixed up with a proof of transaction commitment.
/// To verify that a certain transaction was committed, use a proof from
/// the `block_transactions` index of the [core schema].
///
/// # Verification
///
/// If you are implementing `CallProof` verification, bear in mind that `ExecutionError`
/// need to be stripped off the non-hashed information before verifying `call_proof`.
/// See [`ExecutionError`] docs for more details of what information is not hashed.
///
/// This also means that non-hashed information is not covered by the authenticity guarantees;
/// it has purely diagnostic purpose.
///
/// [transaction]: ../runtime/struct.AnyTx.html
/// [core schema]: struct.Schema.html
/// [`ExecutionError`]: ../runtime/struct.ExecutionError.html
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallProof {
    /// Proof of authenticity for the block header.
    #[serde(flatten)]
    pub block_proof: BlockProof,

    /// Proof of authenticity for the call status. Must contain a single key corresponding
    /// to the `CallInBlock` in question.
    /// The root hash of the proof must be equal to the `error_hash` mentioned in `block_proof`.
    pub call_proof: MapProof<CallInBlock, ExecutionError>,

    #[serde(skip, default)]
    _non_exhaustive: (),
}

impl CallProof {
    pub(super) fn new(
        block_proof: BlockProof,
        call_proof: MapProof<CallInBlock, ExecutionError>,
    ) -> Self {
        Self {
            block_proof,
            call_proof,
            _non_exhaustive: (),
        }
    }

    /// Verifies this proof, returning the location of the proven call together with its status.
    ///
    /// Note that a successful status is indistinguishable from the point of view of a proof
    /// from a non-existing one. It is up to caller to discern between these two possibilities.
    pub fn verify(
        &self,
        validator_keys: &[PublicKey],
    ) -> Result<(CallInBlock, Result<(), ExecutionError>), ProofError> {
        self.block_proof.verify(validator_keys)?;

        // The proof should feature exactly one present or absent entry.
        let mut unchecked_entries = self.call_proof.all_entries_unchecked();
        let (call, maybe_status) = unchecked_entries.next().ok_or(ProofError::NoEntry)?;
        if unchecked_entries.next().is_some() {
            return Err(ProofError::AmbiguousEntry);
        }

        // We create `call_status` before we've removed non-hashed info from the error.
        let call_status = match maybe_status {
            None => Ok(()),
            Some(e) => Err(e.to_owned()),
        };

        self.call_proof
            .clone()
            .map_values(|mut err| {
                // Remove non-hashed part of an error.
                err.split_aux();
                err
            })
            .check_against_hash(self.block_proof.block.error_hash)
            .map_err(ProofError::IncorrectEntryProof)?;
        Ok((call.to_owned(), call_status))
    }
}

#[cfg(test)]
mod tests {
    use assert_matches::assert_matches;
    use chrono::Utc;
    use exonum_crypto::{hash, KeyPair};
    use exonum_merkledb::{
        access::CopyAccessExt, Database, HashTag, ObjectHash, SystemSchema, TemporaryDB,
    };
    use pretty_assertions::{assert_eq, assert_ne};

    use super::*;
    use crate::{blockchain::Schema as CoreSchema, helpers::Round, runtime::InstanceId};

    impl BlockHeaderKey for Hash {
        const NAME: &'static str = "HASH";
        type Value = Self;
    }

    #[test]
    fn block() {
        let mut additional_headers = AdditionalHeaders::new();
        additional_headers.insert::<Hash>(hash(&[0_u8; 10]));

        let txs = [4, 5, 6];
        let height = Height(123_345);
        let prev_hash = hash(&[1, 2, 3]);
        let tx_hash = hash(&txs);
        let tx_count = txs.len() as u32;
        let state_hash = hash(&[7, 8, 9]);

        let error_hash = hash(&[10, 11]);
        let block = Block {
            height,
            tx_count,
            prev_hash,
            tx_hash,
            state_hash,
            error_hash,
            additional_headers,
        };

        let json_str = ::serde_json::to_string(&block).unwrap();
        let block1: Block = ::serde_json::from_str(&json_str).unwrap();
        assert_eq!(block1, block);

        // protobuf roundtrip
        let pb = block.to_pb();
        let de_block: Block = ProtobufConvert::from_pb(pb).unwrap();
        assert_eq!(block, de_block);
    }

    fn create_block(additional_headers: AdditionalHeaders) -> Block {
        let txs = [4, 5, 6];
        let height = Height(123_345);
        let prev_hash = hash(&[1, 2, 3]);
        let tx_hash = hash(&txs);
        let tx_count = txs.len() as u32;
        let state_hash = hash(&[7, 8, 9]);
        let error_hash = hash(&[10, 11]);

        Block {
            height,
            tx_count,
            prev_hash,
            tx_hash,
            state_hash,
            error_hash,
            additional_headers,
        }
    }

    #[test]
    fn block_object_hash() {
        let block_without_entries = create_block(AdditionalHeaders::new());
        let hash_without_entries = block_without_entries.object_hash();

        let mut entries = AdditionalHeaders::new();
        entries.insert::<Hash>(hash(&[0_u8; 10]));

        let block_with_entries = create_block(entries);
        let hash_with_entries = block_with_entries.object_hash();

        assert_ne!(hash_without_entries, hash_with_entries);
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[derive(BinaryValue)]
    #[binary_value(codec = "bincode")]
    struct TestServiceInfo {
        pub instance_id: InstanceId,
        pub runtime_id: u32,
        pub name: String,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
    #[derive(BinaryValue)]
    #[binary_value(codec = "bincode")]
    struct ActiveServices {
        pub services: Vec<TestServiceInfo>,
    }

    impl BlockHeaderKey for ActiveServices {
        const NAME: &'static str = "active_services";
        type Value = Self;
    }

    #[test]
    fn block_entry_keys() {
        let mut block = create_block(AdditionalHeaders::new());
        assert!(block.get_header::<ActiveServices>().unwrap().is_none());

        let services = ActiveServices::default();
        block.add_header::<ActiveServices>(services.clone());
        let restored_services = block
            .get_header::<ActiveServices>()
            .expect("Active services not found");
        assert_eq!(Some(services), restored_services);

        let info = TestServiceInfo {
            runtime_id: 0,
            instance_id: 1,
            name: "test".into(),
        };
        let info_2 = TestServiceInfo {
            runtime_id: 2,
            instance_id: 10,
            name: "test service instance".into(),
        };
        let services = ActiveServices {
            services: vec![info, info_2],
        };

        // Should override previous entry for `ActiveServices`.
        block.add_header::<ActiveServices>(services.clone());
        let restored_services = block
            .get_header::<ActiveServices>()
            .expect("Active services not found");
        assert_eq!(Some(services), restored_services);
    }

    #[test]
    fn block_entry_wrong_type() {
        let mut headers: OrderedMap<String, Vec<u8>> = OrderedMap::default();

        headers
            .0
            .insert("active_services".into(), vec![255_u8; 1_024]);
        let block = create_block(AdditionalHeaders { headers });
        let services = block.get_header::<ActiveServices>();
        assert!(services.is_err());
    }

    fn create_block_proof(keys: &[KeyPair], state_hash: Hash, error_hash: Hash) -> BlockProof {
        let mut block = Block {
            height: Height(1),
            tx_count: 0,
            prev_hash: Hash::zero(),
            tx_hash: Hash::zero(),
            state_hash,
            error_hash,
            additional_headers: AdditionalHeaders::default(),
        };
        block
            .additional_headers
            .insert::<ProposerId>(ValidatorId(1));

        let precommits = keys.iter().enumerate().map(|(i, keypair)| {
            let precommit = Precommit::new(
                ValidatorId(i as u16),
                Height(1),
                Round(1),
                Hash::zero(),
                block.object_hash(),
                Utc::now(),
            );
            Verified::from_value(precommit, keypair.public_key(), keypair.secret_key())
        });
        let precommits = precommits.collect();

        BlockProof::new(block, precommits)
    }

    #[test]
    fn correct_block_proof() {
        let keys: Vec<_> = (0..4).map(|_| KeyPair::random()).collect();
        let public_keys: Vec<_> = keys.iter().map(KeyPair::public_key).collect();

        let mut proof = create_block_proof(&keys, Hash::zero(), Hash::zero());
        proof.verify(&public_keys).unwrap();
        // We can remove one `Precommit` without disturbing the proof integrity.
        proof.precommits.truncate(3);
        proof.verify(&public_keys).unwrap();
    }

    #[test]
    fn incorrect_block_proofs() {
        let keys: Vec<_> = (0..4).map(|_| KeyPair::random()).collect();
        let public_keys: Vec<_> = keys.iter().map(KeyPair::public_key).collect();

        // Too many precommits.
        let proof = create_block_proof(&keys, Hash::zero(), Hash::zero());
        let mut mauled_proof = proof.clone();
        mauled_proof.precommits.push(proof.precommits[0].clone());
        assert_matches!(
            mauled_proof.verify(&public_keys).unwrap_err(),
            ProofError::DoubleEndorsement
        );

        // Too few precommits.
        let mut mauled_proof = proof.clone();
        mauled_proof.precommits.truncate(2);
        assert_matches!(
            mauled_proof.verify(&public_keys).unwrap_err(),
            ProofError::NoQuorum
        );

        // Double endorsement.
        let mut mauled_proof = proof.clone();
        mauled_proof.precommits.truncate(2);
        mauled_proof.precommits.push(proof.precommits[0].clone());
        assert_matches!(
            mauled_proof.verify(&public_keys).unwrap_err(),
            ProofError::DoubleEndorsement
        );

        // Key mismatch.
        let mut expected_public_keys = public_keys.clone();
        expected_public_keys[3] = KeyPair::random().public_key();
        assert_matches!(
            proof.verify(&expected_public_keys).unwrap_err(),
            ProofError::ValidatorKeyMismatch
        );

        // Incorrect height in a precommit.
        let bogus_precommit = Precommit::new(
            ValidatorId(3),
            Height(100),
            Round(1),
            Hash::zero(),
            proof.block.object_hash(),
            Utc::now(),
        );
        let bogus_precommit =
            Verified::from_value(bogus_precommit, public_keys[3], keys[3].secret_key());
        let mut mauled_proof = proof.clone();
        mauled_proof.precommits.truncate(2);
        mauled_proof.precommits.push(bogus_precommit);
        assert_matches!(
            mauled_proof.verify(&public_keys).unwrap_err(),
            ProofError::IncorrectHeight
        );

        // Incorrect block hash in a precommit.
        let bogus_precommit = Precommit::new(
            ValidatorId(3),
            Height(1),
            Round(1),
            Hash::zero(),
            Hash::zero(),
            Utc::now(),
        );
        let bogus_precommit =
            Verified::from_value(bogus_precommit, public_keys[3], keys[3].secret_key());
        let mut mauled_proof = proof;
        mauled_proof.precommits.truncate(2);
        mauled_proof.precommits.push(bogus_precommit);
        assert_matches!(
            mauled_proof.verify(&public_keys).unwrap_err(),
            ProofError::IncorrectBlockHash
        );
    }

    fn create_index_proof() -> (Hash, MapProof<String, Hash>) {
        let db = TemporaryDB::new();
        let fork = db.fork();
        fork.get_proof_list("test.list").extend(vec![1_u8, 2, 3]);
        let patch = fork.into_patch();
        let system_schema = SystemSchema::new(&patch);
        let state_hash = system_schema.state_hash();
        let index_proof = system_schema
            .state_aggregator()
            .get_proof("test.list".to_owned());
        (state_hash, index_proof)
    }

    #[test]
    fn correct_index_proof() {
        let (state_hash, index_proof) = create_index_proof();
        let keys: Vec<_> = (0..4).map(|_| KeyPair::random()).collect();
        let public_keys: Vec<_> = keys.iter().map(KeyPair::public_key).collect();
        let block_proof = create_block_proof(&keys, state_hash, Hash::zero());
        let index_proof = IndexProof::new(block_proof, index_proof);
        let (index_name, index_hash) = index_proof.verify(&public_keys).unwrap();
        assert_eq!(index_name, "test.list");
        let expected_index_hash = HashTag::hash_list(&[1_u8, 2, 3]);
        assert_eq!(index_hash, expected_index_hash);
    }

    #[test]
    fn index_proof_with_incorrect_auth() {
        let (state_hash, index_proof) = create_index_proof();
        let keys: Vec<_> = (0..4).map(|_| KeyPair::random()).collect();
        let public_keys: Vec<_> = keys.iter().map(KeyPair::public_key).collect();
        let block_proof = create_block_proof(&keys, state_hash, Hash::zero());
        let index_proof = IndexProof::new(block_proof, index_proof);

        let mut expected_public_keys = public_keys;
        expected_public_keys.pop();
        expected_public_keys.push(KeyPair::random().public_key());
        assert_matches!(
            index_proof.verify(&expected_public_keys).unwrap_err(),
            ProofError::ValidatorKeyMismatch
        );
    }

    #[test]
    fn index_proof_with_no_index() {
        let db = TemporaryDB::new();
        let snapshot = db.snapshot();
        let system_schema = SystemSchema::new(&snapshot);
        let state_hash = system_schema.state_hash();
        let index_proof = system_schema
            .state_aggregator()
            .get_proof("test.list".to_owned());

        let keys: Vec<_> = (0..4).map(|_| KeyPair::random()).collect();
        let public_keys: Vec<_> = keys.iter().map(KeyPair::public_key).collect();
        let block_proof = create_block_proof(&keys, state_hash, Hash::zero());
        let index_proof = IndexProof::new(block_proof, index_proof);

        assert_matches!(
            index_proof.verify(&public_keys).unwrap_err(),
            ProofError::NoEntry
        );
    }

    #[test]
    fn index_proof_with_multiple_indexes() {
        let db = TemporaryDB::new();
        let fork = db.fork();
        fork.get_proof_entry("test.some").set("!".to_owned());
        fork.get_proof_entry("test.other").set(42_u64);
        let patch = fork.into_patch();
        let system_schema = SystemSchema::new(&patch);
        let state_hash = system_schema.state_hash();
        let index_proof = system_schema
            .state_aggregator()
            .get_multiproof(vec!["test.some".to_owned(), "test.other".to_owned()]);

        let keys: Vec<_> = (0..4).map(|_| KeyPair::random()).collect();
        let public_keys: Vec<_> = keys.iter().map(KeyPair::public_key).collect();
        let block_proof = create_block_proof(&keys, state_hash, Hash::zero());
        let index_proof = IndexProof::new(block_proof, index_proof);

        assert_matches!(
            index_proof.verify(&public_keys).unwrap_err(),
            ProofError::AmbiguousEntry
        );
    }

    #[test]
    fn index_proof_with_mismatched_state_hash() {
        let db = TemporaryDB::new();
        let fork = db.fork();
        fork.get_proof_entry("test.some").set("!".to_owned());
        fork.get_proof_entry("test.other").set(42_u64);
        let patch = fork.into_patch();
        let system_schema = SystemSchema::new(&patch);
        let index_proof = system_schema
            .state_aggregator()
            .get_proof("test.some".to_owned());

        let keys: Vec<_> = (0..4).map(|_| KeyPair::random()).collect();
        let public_keys: Vec<_> = keys.iter().map(KeyPair::public_key).collect();
        let bogus_state_hash = Hash::zero();
        let block_proof = create_block_proof(&keys, bogus_state_hash, Hash::zero());
        let index_proof = IndexProof::new(block_proof, index_proof);

        assert_matches!(
            index_proof.verify(&public_keys).unwrap_err(),
            ProofError::IncorrectEntryProof(ValidationError::UnmatchedRootHash)
        );
    }

    #[derive(Clone, Copy)]
    enum CallProofKind {
        Ok,
        Error,
        Ambiguous,
    }

    fn create_error_proof(kind: CallProofKind) -> (Hash, MapProof<CallInBlock, ExecutionError>) {
        let db = TemporaryDB::new();
        let fork = db.fork();
        let mut schema = CoreSchema::new(&fork);

        let err = ExecutionError::service(5, "huh?");
        let call = CallInBlock::transaction(2);
        schema.save_error(Height(1), call, err.clone());
        let other_call = CallInBlock::after_transactions(0);
        schema.save_error(Height(1), other_call, ExecutionError::service(16, "oops"));

        let error_map = schema.call_errors_map(Height(1));
        let proof = match kind {
            CallProofKind::Ok => error_map.get_proof(CallInBlock::before_transactions(0)),
            CallProofKind::Error => error_map.get_proof(call).map_values(|_| err.clone()),
            CallProofKind::Ambiguous => error_map.get_multiproof(vec![call, other_call]),
        };
        (error_map.object_hash(), proof)
    }

    #[test]
    fn erroneous_call_proof() {
        let (error_hash, call_proof) = create_error_proof(CallProofKind::Error);
        let keys: Vec<_> = (0..4).map(|_| KeyPair::random()).collect();
        let public_keys: Vec<_> = keys.iter().map(KeyPair::public_key).collect();
        let block_proof = create_block_proof(&keys, Hash::zero(), error_hash);
        let mut call_proof = CallProof::new(block_proof, call_proof);
        let (call, res) = call_proof.verify(&public_keys).unwrap();
        assert_eq!(call, CallInBlock::transaction(2));
        assert_eq!(res, Err(ExecutionError::service(5, "huh?")));

        // Check that the proof remains valid if we remove or change the description.
        call_proof.call_proof = call_proof
            .call_proof
            .map_values(|_| ExecutionError::service(5, ""));
        let _ = call_proof.verify(&public_keys).unwrap();
        call_proof.call_proof = call_proof
            .call_proof
            .map_values(|_| ExecutionError::service(5, "?huh"));
        let _ = call_proof.verify(&public_keys).unwrap();

        // ...but not if we change the hashed part of the error.
        call_proof.call_proof = call_proof
            .call_proof
            .map_values(|_| ExecutionError::service(6, "huh?"));
        let err = call_proof.verify(&public_keys).unwrap_err();
        assert_matches!(
            err,
            ProofError::IncorrectEntryProof(ValidationError::UnmatchedRootHash)
        );
    }

    #[test]
    fn ok_call_proof() {
        let (error_hash, call_proof) = create_error_proof(CallProofKind::Ok);
        let keys: Vec<_> = (0..3).map(|_| KeyPair::random()).collect();
        let public_keys: Vec<_> = keys.iter().map(KeyPair::public_key).collect();
        let block_proof = create_block_proof(&keys, Hash::zero(), error_hash);
        let mut call_proof = CallProof::new(block_proof, call_proof);
        let (call, res) = call_proof.verify(&public_keys).unwrap();
        assert_eq!(call, CallInBlock::before_transactions(0));
        assert_eq!(res, Ok(()));

        // Check proof invalidation if the block part is mangled.
        call_proof.block_proof.block.height = Height(100);
        let err = call_proof.verify(&public_keys).unwrap_err();
        assert_matches!(err, ProofError::IncorrectHeight);
    }

    #[test]
    fn ambiguous_call_proof() {
        let (error_hash, call_proof) = create_error_proof(CallProofKind::Ambiguous);
        let keys: Vec<_> = (0..3).map(|_| KeyPair::random()).collect();
        let public_keys: Vec<_> = keys.iter().map(KeyPair::public_key).collect();
        let block_proof = create_block_proof(&keys, Hash::zero(), error_hash);
        let call_proof = CallProof::new(block_proof, call_proof);
        let err = call_proof.verify(&public_keys).unwrap_err();
        assert_matches!(err, ProofError::AmbiguousEntry);
    }
}

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

use assert_matches::assert_matches;
use exonum::{
    blockchain::{
        config::GenesisConfigBuilder, Blockchain, BlockchainBuilder, BlockchainMut,
        ConsensusConfig, IndexProof,
    },
    crypto::PublicKey,
    helpers::{Height, ValidatorId},
    merkledb::{
        access::{Access, CopyAccessExt, FromAccess, Prefixed},
        Entry, HashTag, ProofMapIndex, Snapshot,
    },
    runtime::{
        versioning::{ArtifactReq, ArtifactReqError, RequireArtifact},
        BlockchainData, InstanceDescriptor, SnapshotExt,
    },
};
use exonum_derive::{FromAccess, ServiceDispatcher, ServiceFactory};
use futures::sync::mpsc;

use std::collections::BTreeMap;

use exonum_rust_runtime::{DefaultInstance, RustRuntimeBuilder, Service, ServiceFactory};

#[derive(Debug, FromAccess)]
struct Schema<T: Access> {
    pub wallets: ProofMapIndex<T::Base, PublicKey, u64>,
}

impl<T: Access> RequireArtifact for Schema<T> {
    fn required_artifact() -> ArtifactReq {
        "exonum.Token@^1.3.0".parse().unwrap()
    }
}

#[derive(Debug, FromAccess)]
struct SchemaImpl<T: Access> {
    #[from_access(flatten)]
    public: Schema<T>,
    private: Entry<T::Base, String>,
}

#[derive(Debug, ServiceDispatcher, ServiceFactory)]
#[service_factory(artifact_name = "exonum.Token", artifact_version = "1.4.0")]
struct TokenService;

impl Service for TokenService {}

impl DefaultInstance for TokenService {
    const INSTANCE_ID: u32 = 100;
    const INSTANCE_NAME: &'static str = "token";
}

#[derive(Debug, ServiceDispatcher, ServiceFactory)]
#[service_factory(artifact_name = "exonum.Token", artifact_version = "1.0.0")]
struct OldService;

impl Service for OldService {}

impl DefaultInstance for OldService {
    const INSTANCE_ID: u32 = 101;
    const INSTANCE_NAME: &'static str = "old-token";
}

#[derive(Debug, ServiceDispatcher, ServiceFactory)]
#[service_factory(artifact_name = "exonum.OtherService", artifact_version = "1.3.5")]
struct OtherService;

impl Service for OtherService {}

impl DefaultInstance for OtherService {
    const INSTANCE_ID: u32 = 102;
    const INSTANCE_NAME: &'static str = "other";
}

fn create_blockchain() -> BlockchainMut {
    let (config, _) = ConsensusConfig::for_tests(1);
    let genesis_config = GenesisConfigBuilder::with_consensus_config(config)
        .with_artifact(TokenService.artifact_id())
        .with_instance(TokenService.default_instance())
        .with_artifact(OldService.artifact_id())
        .with_instance(OldService.default_instance())
        .with_artifact(OtherService.artifact_id())
        .with_instance(OtherService.default_instance())
        .build();

    let runtime = RustRuntimeBuilder::new()
        .with_factory(TokenService)
        .with_factory(OldService)
        .with_factory(OtherService)
        .build(mpsc::channel(1).0);

    BlockchainBuilder::new(Blockchain::build_for_tests())
        .with_genesis_config(genesis_config)
        .with_runtime(runtime)
        .build()
}

fn setup_blockchain_for_index_proofs() -> Box<dyn Snapshot> {
    let mut blockchain = create_blockchain();
    let fork = blockchain.fork();
    fork.get_proof_list("test.list").push(1_u32);
    fork.get_proof_entry(("test.entry", &0_u8))
        .set("!".to_owned());
    fork.get_value_set("test.set").insert(2_u64);
    blockchain.merge(fork.into_patch()).unwrap();

    let (block_hash, patch) =
        blockchain.create_patch(ValidatorId(0), Height(1), &[], &mut BTreeMap::new());
    blockchain
        .commit(patch, block_hash, vec![], &mut BTreeMap::new())
        .unwrap();
    blockchain.snapshot()
}

fn check_list_proof(proof: &IndexProof) {
    let block = &proof.block_proof.block;
    assert_eq!(block.height, Height(1));
    let checked_proof = proof
        .index_proof
        .check_against_hash(block.state_hash)
        .unwrap();
    let entries: Vec<_> = checked_proof
        .entries()
        .map(|(name, hash)| (name.as_str(), *hash))
        .collect();
    assert_eq!(entries, vec![("test.list", HashTag::hash_list(&[1_u32]))]);
}

#[test]
fn proof_for_index_in_snapshot() {
    let snapshot = setup_blockchain_for_index_proofs();
    let proof = snapshot.proof_for_index("test.list").unwrap();
    check_list_proof(&proof);
    // Since the entry has non-empty ID in group, a proof for it should not be returned.
    assert!(snapshot.proof_for_index("test.entry").is_none());
    // Value sets are not Merkelized.
    assert!(snapshot.proof_for_index("test.set").is_none());
}

#[test]
fn proof_for_service_index() {
    let snapshot = setup_blockchain_for_index_proofs();
    let instance = InstanceDescriptor::new(100, "test");
    let data = BlockchainData::new(snapshot.as_ref(), &instance.name);
    let proof = data.proof_for_service_index("list").unwrap();
    check_list_proof(&proof);
    assert!(data.proof_for_service_index("entry").is_none());
    assert!(data.proof_for_service_index("set").is_none());
}

#[test]
fn access_to_service_schema() {
    let mut blockchain = create_blockchain();
    let fork = blockchain.fork();
    {
        let mut schema = SchemaImpl::from_root(Prefixed::new("token", &fork)).unwrap();
        schema.public.wallets.put(&PublicKey::new([0; 32]), 100);
        schema.public.wallets.put(&PublicKey::new([1; 32]), 200);
        schema.private.set("Some value".to_owned());
    }

    let instance = InstanceDescriptor::new(0, "who");
    let data = BlockchainData::new(&fork, &instance.name);
    {
        let schema: Schema<_> = data.service_schema("token").unwrap();
        assert_eq!(schema.wallets.values().sum::<u64>(), 300);
    }

    let err = data
        .service_schema::<Schema<_>, _>("what")
        .expect_err("Retrieving schema for non-existing service should fail");
    assert_matches!(err, ArtifactReqError::NoService);
    let err = data
        .service_schema::<Schema<_>, _>("old-token")
        .expect_err("Retrieving schema for old service should fail");
    assert_matches!(err, ArtifactReqError::IncompatibleVersion { .. });
    let err = data
        .service_schema::<Schema<_>, _>("other")
        .expect_err("Retrieving schema for unrelated service should fail");
    assert_matches!(
        err,
        ArtifactReqError::UnexpectedName { ref actual, .. } if actual == "exonum.OtherService"
    );

    blockchain.merge(fork.into_patch()).unwrap();
    let snapshot = blockchain.snapshot();
    let schema: Schema<_> = snapshot.service_schema("token").unwrap();
    assert_eq!(schema.wallets.values().sum::<u64>(), 300);
}

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

use exonum_merkledb::{
    access::{AsReadonly, FromAccess, Prefixed, RawAccess},
    Snapshot, SystemSchema,
};

use super::{
    versioning::{ArtifactReqError, RequireArtifact},
    DispatcherSchema, InstanceDescriptor, InstanceQuery, InstanceSpec, InstanceStatus,
};
use crate::blockchain::{IndexProof, Schema as CoreSchema};

/// Provides access to blockchain data for the executing service.
#[derive(Debug, Clone, Copy)]
pub struct BlockchainData<'a, T> {
    access: T,
    service_instance: InstanceDescriptor<'a>,
}

impl<'a, T: RawAccess + AsReadonly> BlockchainData<'a, T> {
    /// Creates structured access to blockchain data based on the unstructured access
    /// (e.g., a `Snapshot` or a `Fork`) and the descriptor of the executing service.
    pub fn new(access: T, service_instance: InstanceDescriptor<'a>) -> Self {
        Self {
            access,
            service_instance,
        }
    }

    /// Returns core schema.
    pub fn for_core(&self) -> CoreSchema<T::Readonly> {
        CoreSchema::new(self.access.as_readonly())
    }

    /// Returns dispatcher schema.
    pub fn for_dispatcher(&self) -> DispatcherSchema<T::Readonly> {
        DispatcherSchema::new(self.access.as_readonly())
    }

    /// Returns a mount point for another service. If the service with `id` does not exist,
    /// returns `None`.
    ///
    /// # Safety
    ///
    /// This method does not check the service type; the caller is responsible
    /// for constructing a schema of a correct type around the returned access. Constructing
    /// an incorrect schema can lead to a panic or unexpected behavior. Use [`service_schema`]
    /// as a safer alternative, which performs all necessary checks.
    ///
    /// [`service_schema`]: #method.service_schema
    pub fn for_service<'q>(
        &self,
        id: impl Into<InstanceQuery<'q>>,
    ) -> Option<Prefixed<'static, T::Readonly>> {
        mount_point_for_service(self.access.as_readonly(), id).map(|(access, _)| access)
    }

    /// Retrieves schema for a service.
    ///
    /// # Errors
    ///
    /// Returns an error in the following situations (see [`ArtifactReqError`] for more details):
    ///
    /// - Service with the given ID does not exist
    /// - Service has an unexpected artifact name
    /// - Service has an incompatible artifact version
    ///
    /// [`ArtifactReqError`]: versioning/enum.ArtifactReqError.html
    pub fn service_schema<'q, S, I>(&self, service_id: I) -> Result<S, ArtifactReqError>
    where
        S: RequireArtifact + FromAccess<Prefixed<'static, T::Readonly>>,
        I: Into<InstanceQuery<'q>>,
    {
        schema_for_service(self.access.as_readonly(), service_id)
    }

    /// Returns a mount point for the data of the executing service instance.
    /// Unlike other data, this one may be writeable provided that this `BlockchainData`
    /// wraps a `Fork`.
    pub fn for_executing_service(&self) -> Prefixed<'a, T> {
        Prefixed::new(self.service_instance.name, self.access.clone())
    }
}

impl BlockchainData<'_, &dyn Snapshot> {
    /// Returns a proof for a Merkelized index with the specified name
    /// in the currently executing service.
    ///
    /// # Return value
    ///
    /// If the index does not exist or is not Merkelized, returns `None`. Note that this may
    /// occur before the index is accessed the first time, which, depending on the service logic,
    /// may happen indefinitely after the service has been initialized. Thus, `unwrap`ping the
    /// returned value may unexpectedly lead to a panic unless the index is initialized early
    /// (e.g., during service initialization).
    pub fn proof_for_service_index(&self, index_name: &str) -> Option<IndexProof> {
        let full_index_name = [self.service_instance.name, ".", index_name].concat();
        self.access.proof_for_index(&full_index_name)
    }
}

fn mount_point_for_service<'q, T: RawAccess>(
    access: T,
    id: impl Into<InstanceQuery<'q>>,
) -> Option<(Prefixed<'static, T>, InstanceSpec)> {
    let state = DispatcherSchema::new(access.clone())
        .get_instance(id)
        .filter(|state| match (state.status, state.pending_status) {
            (Some(InstanceStatus::Active), _) | (None, Some(InstanceStatus::Active)) => true,
            _ => false,
        })?;
    Some((Prefixed::new(state.spec.name.clone(), access), state.spec))
}

fn schema_for_service<'q, T, S>(
    access: T,
    service_id: impl Into<InstanceQuery<'q>>,
) -> Result<S, ArtifactReqError>
where
    T: RawAccess,
    S: RequireArtifact + FromAccess<Prefixed<'static, T>>,
{
    let (access, spec) =
        mount_point_for_service(access, service_id).ok_or(ArtifactReqError::NoService)?;

    let artifact_req = S::required_artifact();
    artifact_req.try_match(&spec.artifact)?;
    Ok(S::from_root(access).unwrap())
}

/// Extension trait for `Snapshot` allowing to access blockchain data in a more structured way.
pub trait SnapshotExt {
    /// Returns core schema.
    fn for_core(&self) -> CoreSchema<&'_ dyn Snapshot>;
    /// Returns dispatcher schema.
    fn for_dispatcher(&self) -> DispatcherSchema<&'_ dyn Snapshot>;

    /// Returns a mount point for a service. If the service does not exist, returns `None`.
    ///
    /// # Safety
    ///
    /// This method does not check the service type; the caller is responsible
    /// for constructing a schema of a correct type around the returned access. Constructing
    /// an incorrect schema can lead to a panic or unexpected behavior. Use [`service_schema`]
    /// as a safer alternative, which performs all necessary checks.
    ///
    /// [`service_schema`]: #tymethod.service_schema
    fn for_service<'q>(
        &self,
        id: impl Into<InstanceQuery<'q>>,
    ) -> Option<Prefixed<'static, &dyn Snapshot>>;

    /// Returns a proof for a Merkelized index with the specified name.
    ///
    /// # Return value
    ///
    /// If the index does not exist or is not Merkelized, returns `None`. Note that this may
    /// occur before the index is accessed the first time, which, depending on the service logic,
    /// may happen indefinitely after the service has been initialized. Thus, `unwrap`ping the
    /// returned value may unexpectedly lead to a panic unless the index is initialized early
    /// (e.g., during service initialization).
    #[doc(hidden)]
    fn proof_for_index(&self, index_name: &str) -> Option<IndexProof>;

    /// Retrieves schema for a service.
    ///
    /// # Errors
    ///
    /// Returns an error in the following situations (see [`ArtifactReqError`] for more details):
    ///
    /// - Service with the given ID does not exist
    /// - Service has an unexpected artifact name
    /// - Service has an incompatible artifact version
    ///
    /// [`ArtifactReqError`]: versioning/enum.ArtifactReqError.html
    fn service_schema<'s, 'q, S, I>(&'s self, service_id: I) -> Result<S, ArtifactReqError>
    where
        S: RequireArtifact + FromAccess<Prefixed<'static, &'s dyn Snapshot>>,
        I: Into<InstanceQuery<'q>>;
}

impl SnapshotExt for dyn Snapshot {
    fn for_core(&self) -> CoreSchema<&'_ dyn Snapshot> {
        CoreSchema::new(self)
    }

    fn for_dispatcher(&self) -> DispatcherSchema<&'_ dyn Snapshot> {
        DispatcherSchema::new(self)
    }

    fn for_service<'q>(
        &self,
        id: impl Into<InstanceQuery<'q>>,
    ) -> Option<Prefixed<'static, &dyn Snapshot>> {
        mount_point_for_service(self, id).map(|(access, _)| access)
    }

    fn proof_for_index(&self, index_name: &str) -> Option<IndexProof> {
        let core_schema = self.for_core();
        let height = core_schema.height();
        let block_proof = core_schema.block_and_precommits(height).unwrap();

        let aggregator = SystemSchema::new(self).state_aggregator();
        aggregator.get(index_name)?;
        let index_proof = aggregator.get_proof(index_name.to_string());
        Some(IndexProof {
            block_proof,
            index_proof,
        })
    }

    fn service_schema<'s, 'q, S, I>(&'s self, service_id: I) -> Result<S, ArtifactReqError>
    where
        S: RequireArtifact + FromAccess<Prefixed<'static, &'s dyn Snapshot>>,
        I: Into<InstanceQuery<'q>>,
    {
        schema_for_service(self, service_id)
    }
}

#[cfg(test)]
mod tests {
    use assert_matches::assert_matches;
    use exonum_crypto::PublicKey;
    use exonum_derive::*;
    use exonum_merkledb::{
        access::{Access, AccessExt, FromAccess},
        Entry, HashTag, ProofMapIndex,
    };

    use std::collections::BTreeMap;

    use super::*;
    use crate::runtime::versioning::ArtifactReq;
    use crate::{
        blockchain::config::GenesisConfigBuilder,
        blockchain::{Blockchain, BlockchainMut},
        helpers::{generate_testnet_config, Height, ValidatorId},
        runtime::rust::{DefaultInstance, RustRuntime, Service, ServiceFactory},
    };

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
    #[service_factory(
        crate = "crate",
        artifact_name = "exonum.Token",
        artifact_version = "1.4.0"
    )]
    #[service_dispatcher(crate = "crate")]
    struct TokenService;

    impl Service for TokenService {}

    impl DefaultInstance for TokenService {
        const INSTANCE_ID: u32 = 100;
        const INSTANCE_NAME: &'static str = "token";
    }

    #[derive(Debug, ServiceDispatcher, ServiceFactory)]
    #[service_factory(
        crate = "crate",
        artifact_name = "exonum.Token",
        artifact_version = "1.0.0"
    )]
    #[service_dispatcher(crate = "crate")]
    struct OldService;

    impl Service for OldService {}

    impl DefaultInstance for OldService {
        const INSTANCE_ID: u32 = 101;
        const INSTANCE_NAME: &'static str = "old-token";
    }

    #[derive(Debug, ServiceDispatcher, ServiceFactory)]
    #[service_factory(
        crate = "crate",
        artifact_name = "exonum.OtherService",
        artifact_version = "1.3.5"
    )]
    #[service_dispatcher(crate = "crate")]
    struct OtherService;

    impl Service for OtherService {}

    impl DefaultInstance for OtherService {
        const INSTANCE_ID: u32 = 102;
        const INSTANCE_NAME: &'static str = "other";
    }

    fn create_blockchain() -> BlockchainMut {
        let config = generate_testnet_config(1, 0)[0].clone();
        let genesis_config = GenesisConfigBuilder::with_consensus_config(config.consensus)
            .with_artifact(TokenService.artifact_id())
            .with_instance(TokenService.default_instance())
            .with_artifact(OldService.artifact_id())
            .with_instance(OldService.default_instance())
            .with_artifact(OtherService.artifact_id())
            .with_instance(OtherService.default_instance())
            .build();

        let runtime = RustRuntime::builder()
            .with_factory(TokenService)
            .with_factory(OldService)
            .with_factory(OtherService);

        Blockchain::build_for_tests()
            .into_mut(genesis_config)
            .with_runtime(runtime.build_for_tests())
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
            blockchain.create_patch(ValidatorId(0).into(), Height(1), &[], &mut BTreeMap::new());
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
        let instance = InstanceDescriptor {
            id: 100,
            name: "test",
        };
        let data = BlockchainData::new(snapshot.as_ref(), instance);
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

        let instance = InstanceDescriptor { id: 0, name: "who" };
        let data = BlockchainData::new(&fork, instance);
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
}

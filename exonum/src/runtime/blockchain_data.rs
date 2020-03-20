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
    generic::GenericRawAccess,
    Snapshot, SystemSchema,
};

use super::{
    versioning::{ArtifactReqError, RequireArtifact},
    DispatcherSchema, InstanceQuery, InstanceSpec, InstanceState,
};
use crate::blockchain::{IndexProof, Schema as CoreSchema};

/// Provides access to blockchain data for the executing service.
#[derive(Debug, Clone)]
pub struct BlockchainData<T> {
    access: T,
    instance_name: String,
}

impl<T: RawAccess + AsReadonly> BlockchainData<T> {
    /// Creates structured access to blockchain data based on the unstructured access
    /// (e.g., a `Snapshot` or a `Fork`) and the descriptor of the executing service.
    pub fn new(access: T, instance_name: impl Into<String>) -> Self {
        Self {
            access,
            instance_name: instance_name.into(),
        }
    }

    /// Returns the instance name of the executing service.
    pub fn instance_name(&self) -> &str {
        &self.instance_name
    }

    /// Returns unstructured readonly access to blockchain data.
    ///
    /// # Safety
    ///
    /// Trying to access data of the executing service through the returned access
    /// can lead to a panic because of borrowing checks performed by the database.
    /// Check that your wrapper prevent such attempts to access.
    #[doc(hidden)]
    pub fn unstructured_access(&self) -> T::Readonly {
        self.access.as_readonly()
    }

    /// Returns core schema.
    pub fn for_core(&self) -> CoreSchema<T::Readonly> {
        CoreSchema::new(self.unstructured_access())
    }

    /// Returns dispatcher schema.
    pub fn for_dispatcher(&self) -> DispatcherSchema<T::Readonly> {
        DispatcherSchema::new(self.unstructured_access())
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
    ) -> Option<Prefixed<T::Readonly>> {
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
        S: RequireArtifact + FromAccess<Prefixed<T::Readonly>>,
        I: Into<InstanceQuery<'q>>,
    {
        schema_for_service(self.access.as_readonly(), service_id)
    }

    /// Returns a mount point for the data of the executing service instance.
    /// Unlike other data, this one may be writeable provided that this `BlockchainData`
    /// wraps a `Fork`.
    pub fn for_executing_service(&self) -> Prefixed<T> {
        Prefixed::new(&self.instance_name, self.access.clone())
    }
}

impl BlockchainData<&dyn Snapshot> {
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
        let full_index_name = [&self.instance_name, ".", index_name].concat();
        self.access.proof_for_index(&full_index_name)
    }
}

#[allow(clippy::use_self)] // false positive
impl<'a, T> BlockchainData<T>
where
    T: Into<GenericRawAccess<'a>>,
{
    /// Erases the enclosed access, converting it to the generic form.
    pub fn erase_access(self) -> BlockchainData<GenericRawAccess<'a>> {
        BlockchainData::new(self.access.into(), self.instance_name)
    }
}

fn mount_point_for_service<'q, T: RawAccess>(
    access: T,
    id: impl Into<InstanceQuery<'q>>,
) -> Option<(Prefixed<T>, InstanceSpec)> {
    let state = DispatcherSchema::new(access.clone())
        .get_instance(id)
        .filter(InstanceState::is_readable)?;
    Some((Prefixed::new(state.spec.name.clone(), access), state.spec))
}

fn schema_for_service<'q, T, S>(
    access: T,
    service_id: impl Into<InstanceQuery<'q>>,
) -> Result<S, ArtifactReqError>
where
    T: RawAccess,
    S: RequireArtifact + FromAccess<Prefixed<T>>,
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
    fn for_service<'q>(&self, id: impl Into<InstanceQuery<'q>>) -> Option<Prefixed<&dyn Snapshot>>;

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
        S: RequireArtifact + FromAccess<Prefixed<&'s dyn Snapshot>>,
        I: Into<InstanceQuery<'q>>;
}

impl SnapshotExt for dyn Snapshot {
    fn for_core(&self) -> CoreSchema<&'_ dyn Snapshot> {
        CoreSchema::new(self)
    }

    fn for_dispatcher(&self) -> DispatcherSchema<&'_ dyn Snapshot> {
        DispatcherSchema::new(self)
    }

    fn for_service<'q>(&self, id: impl Into<InstanceQuery<'q>>) -> Option<Prefixed<&dyn Snapshot>> {
        mount_point_for_service(self, id).map(|(access, _)| access)
    }

    fn proof_for_index(&self, index_name: &str) -> Option<IndexProof> {
        let core_schema = self.for_core();
        let height = core_schema.height();
        let block_proof = core_schema.block_and_precommits(height).unwrap();

        let aggregator = SystemSchema::new(self).state_aggregator();
        aggregator.get(index_name)?;
        let index_proof = aggregator.get_proof(index_name.to_string());
        Some(IndexProof::new(block_proof, index_proof))
    }

    fn service_schema<'s, 'q, S, I>(&'s self, service_id: I) -> Result<S, ArtifactReqError>
    where
        S: RequireArtifact + FromAccess<Prefixed<&'s dyn Snapshot>>,
        I: Into<InstanceQuery<'q>>,
    {
        schema_for_service(self, service_id)
    }
}

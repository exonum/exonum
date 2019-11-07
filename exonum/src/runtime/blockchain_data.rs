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

use exonum_merkledb::{
    access::{Prefixed, RawAccess, ToReadonly},
    Snapshot,
};

use super::{DispatcherSchema, InstanceDescriptor, InstanceQuery};
use crate::blockchain::Schema as CoreSchema;

/// Provides access to blockchain data for the executing service.
#[derive(Debug, Clone, Copy)]
pub struct BlockchainData<'a, T> {
    access: T,
    service_instance: InstanceDescriptor<'a>,
}

impl<'a, T: RawAccess + ToReadonly> BlockchainData<'a, T> {
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
        CoreSchema::new(self.access.to_readonly())
    }

    /// Returns dispatcher schema.
    pub fn for_dispatcher(&self) -> DispatcherSchema<T::Readonly> {
        DispatcherSchema::new(self.access.to_readonly())
    }

    /// Returns a mount point for another service. If the service with `id` does not exist,
    /// returns `None`.
    ///
    /// Note that this method does not check the service type; the caller is responsible
    /// for constructing a schema of a correct type around the returned access. Constructing
    /// an incorrect schema can lead to a panic or unexpected behavior.
    pub fn for_service<'q>(
        &self,
        id: impl Into<InstanceQuery<'q>>,
    ) -> Option<Prefixed<'static, T::Readonly>> {
        mount_point_for_service(self.access.to_readonly(), id)
    }

    /// Returns a mount point for the data of the executing service instance.
    /// Unlike other data, this one may be writeable provided that this `BlockchainData`
    /// wraps a `Fork`.
    pub fn for_executing_service(&self) -> Prefixed<'a, T> {
        Prefixed::new(self.service_instance.name, self.access.clone())
    }
}

fn mount_point_for_service<'q, T: RawAccess>(
    access: T,
    id: impl Into<InstanceQuery<'q>>,
) -> Option<Prefixed<'static, T>> {
    let (spec, _) = DispatcherSchema::new(access.clone()).get_instance(id)?;
    Some(Prefixed::new(spec.name, access))
}

/// Extension trait for `Snapshot` allowing to access blockchain data in a more structured way.
pub trait SnapshotExt {
    /// Returns core schema.
    fn for_core(&self) -> CoreSchema<&'_ dyn Snapshot>;
    /// Returns dispatcher schema.
    fn for_dispatcher(&self) -> DispatcherSchema<&'_ dyn Snapshot>;
    /// Returns a mount point for a service. If the service does not exist, returns `None`.
    fn for_service<'q>(
        &self,
        id: impl Into<InstanceQuery<'q>>,
    ) -> Option<Prefixed<'static, &dyn Snapshot>>;
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
        mount_point_for_service(self, id)
    }
}

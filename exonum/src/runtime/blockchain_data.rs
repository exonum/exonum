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

use exonum_merkledb::{IndexAccess, Prefixed, Snapshot, ToReadonly};

use super::{DeployStatus, DispatcherSchema, InstanceDescriptor, InstanceQuery};
use crate::blockchain::Schema as CoreSchema;

/// Provides access to blockchain data for the executing service.
#[derive(Debug, Clone, Copy)]
pub struct BlockchainData<'a, T> {
    access: T,
    service_instance: InstanceDescriptor<'a>,
}

impl<'a, T: IndexAccess + ToReadonly> BlockchainData<'a, T> {
    pub(super) fn new(access: T, service_instance: InstanceDescriptor<'a>) -> Self {
        Self {
            access,
            service_instance,
        }
    }

    /// Provides full access to entire storage. This is currently used by the sandbox.
    #[cfg(test)]
    pub(crate) fn full_access_to_everything(&self) -> T {
        self.access.clone()
    }

    /// Returns core schema.
    pub fn core_schema(&self) -> CoreSchema<T::Readonly> {
        CoreSchema::get_unchecked(self.access.to_readonly())
    }

    /// Returns dispatcher schema.
    pub fn for_dispatcher(&self) -> DispatcherSchema<T::Readonly> {
        DispatcherSchema::new(self.access.to_readonly())
    }

    /// Returns a mount point for another service.
    pub fn for_service<'q>(
        &self,
        id: impl Into<InstanceQuery<'q>>,
    ) -> Option<Prefixed<'_, T::Readonly>> {
        // The returned value is `Prefixed<'static, _>`, but we coerce it to a shorter lifetime
        // for future compatibility.
        mount_point_for_service(self.access.to_readonly(), id)
    }

    /// Returns a mount point for the data of the executing service instance.
    /// Unlike other data, this one may be writeable provided that this `BlockchainData`
    /// wraps a `Fork`.
    pub fn for_executing_service(&self) -> Prefixed<'a, T> {
        Prefixed::new(self.service_instance.name, self.access.clone())
    }
}

fn mount_point_for_service<'q, T: IndexAccess>(
    access: T,
    id: impl Into<InstanceQuery<'q>>,
) -> Option<Prefixed<'static, T>> {
    let (spec, status) = DispatcherSchema::new(access.clone()).get_instance(id)?;
    if status != DeployStatus::Active {
        return None;
    }
    Some(Prefixed::new(spec.name, access))
}

/// Extension trait for `Snapshot` allowing to access core and service schemas.
pub trait SnapshotExt {
    /// Returns core schema.
    fn core_schema(&self) -> CoreSchema<&'_ dyn Snapshot>;
    /// Returns dispatcher schema.
    fn for_dispatcher(&self) -> DispatcherSchema<&'_ dyn Snapshot>;
    /// Returns a mount point for a service. If the service does not exist, returns `None`.
    fn for_service<'q>(
        &self,
        id: impl Into<InstanceQuery<'q>>,
    ) -> Option<Prefixed<'_, &'_ dyn Snapshot>>;
}

impl SnapshotExt for dyn Snapshot {
    fn core_schema(&self) -> CoreSchema<&'_ dyn Snapshot> {
        CoreSchema::get_unchecked(self)
    }

    fn for_dispatcher(&self) -> DispatcherSchema<&'_ dyn Snapshot> {
        DispatcherSchema::new(self)
    }

    fn for_service<'q>(
        &self,
        id: impl Into<InstanceQuery<'q>>,
    ) -> Option<Prefixed<'_, &'_ dyn Snapshot>> {
        // The returned value is `Prefixed<'static, _>`, but we coerce it to a shorter lifetime
        // for future compatibility.
        mount_point_for_service(self, id)
    }
}

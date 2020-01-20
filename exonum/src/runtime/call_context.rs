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

use crate::{
    blockchain::Schema as CoreSchema,
    crypto::Hash,
    helpers::Height,
    merkledb::{access::Prefixed, BinaryValue, Fork},
    runtime::{
        ArtifactId, BlockchainData, CallInfo, CallSite, CallType, Caller, CoreError, Dispatcher,
        DispatcherSchema, ExecutionContext, ExecutionContextUnstable, ExecutionError,
        InstanceDescriptor, InstanceId, InstanceQuery, InstanceSpec, InstanceStatus,
        SUPERVISOR_INSTANCE_ID,
    },
};

/// Context for the executed call.
///
/// The call can mean a transaction call, `before_transactions` / `after_transactions` hook,
/// or the service constructor invocation.
#[derive(Debug)]
pub struct CallContext<'a> {
    /// Underlying execution context.
    inner: ExecutionContext<'a>,
    /// ID of the executing service.
    instance: InstanceDescriptor<'a>,
}

impl<'a> CallContext<'a> {
    /// Creates a new transaction context for the specified execution context and the instance
    /// descriptor.
    pub fn new(context: ExecutionContext<'a>, instance: InstanceDescriptor<'a>) -> Self {
        Self {
            inner: context,
            instance,
        }
    }

    /// Provides access to blockchain data.
    pub fn data(&self) -> BlockchainData<'a, &Fork> {
        BlockchainData::new(self.inner.fork, self.instance)
    }

    /// Provides access to the data of the executing service.
    pub fn service_data(&self) -> Prefixed<'a, &Fork> {
        self.data().for_executing_service()
    }

    /// Returns the authorization information about this call.
    pub fn caller(&self) -> &Caller {
        &self.inner.caller
    }

    /// Returns the hash of the currently executing transaction, or `None` for non-transaction
    /// root calls (e.g., `before_transactions` / `after_transactions` service hooks).
    pub fn transaction_hash(&self) -> Option<Hash> {
        self.inner.transaction_hash()
    }

    /// Returns a descriptor of the executing service instance.
    pub fn instance(&self) -> InstanceDescriptor<'_> {
        self.instance
    }

    /// Returns `true` if currently processed block is a genesis block.
    pub fn in_genesis_block(&self) -> bool {
        let core_schema = self.data().for_core();
        core_schema.next_height() == Height(0)
    }

    /// Returns extensions required for the Supervisor service implementation.
    ///
    /// This method can only be called by the supervisor; the call will panic otherwise.
    #[doc(hidden)]
    pub fn supervisor_extensions(&mut self) -> SupervisorExtensions<'_> {
        if self.instance.id != SUPERVISOR_INSTANCE_ID {
            panic!("`supervisor_extensions` called within a non-supervisor service");
        }
        self.inner.supervisor_extensions()
    }
}

/// Collection of unstable call context features.
#[doc(hidden)]
pub trait CallContextUnstable {
    /// Re-borrows an call context with the same interface name.
    fn reborrow(&mut self) -> CallContext<'_>;
    /// Returns the service matching the specified query.
    fn get_service<'q>(&self, id: impl Into<InstanceQuery<'q>>) -> Option<InstanceDescriptor<'_>>;
    /// Invokes the interface method of the instance with the specified ID.
    /// You may override the instance ID of the one who calls this method by the given one.
    fn make_child_call(
        &mut self,
        interface_name: &str,
        call_info: &CallInfo,
        arguments: &[u8],
        caller: Option<InstanceId>,
    ) -> Result<(), ExecutionError>;
}

impl<'a> CallContextUnstable for CallContext<'a> {
    fn reborrow(&mut self) -> CallContext<'_> {
        CallContext::new(self.inner.reborrow(), self.instance)
    }

    fn get_service<'q>(&self, id: impl Into<InstanceQuery<'q>>) -> Option<InstanceDescriptor<'_>> {
        self.inner.get_service(id)
    }

    fn make_child_call(
        &mut self,
        interface_name: &str,
        call_info: &CallInfo,
        arguments: &[u8],
        caller: Option<InstanceId>,
    ) -> Result<(), ExecutionError> {
        self.inner
            .make_child_call(interface_name, call_info, arguments, caller)
    }
}

/// Execution context extensions required for the Supervisor service implementation.
#[doc(hidden)]
#[derive(Debug)]
pub struct SupervisorExtensions<'a>(pub(super) ExecutionContext<'a>);

impl<'a> SupervisorExtensions<'a> {
    /// Marks an artifact as *committed*, i.e., one which service instances can be deployed from.
    ///
    /// If / when a block with this instruction is accepted, artifact deployment becomes
    /// a requirement for all nodes in the network. A node that did not successfully
    /// deploy the artifact previously blocks until the artifact is deployed successfully.
    /// If a node cannot deploy the artifact, it panics.
    pub fn start_artifact_registration(&self, artifact: ArtifactId, spec: Vec<u8>) {
        Dispatcher::commit_artifact(self.0.fork, artifact, spec);
    }

    /// Initiates adding a service instance to the blockchain.
    ///
    /// The service is not immediately activated; it activates if / when the block containing
    /// the activation transaction is committed.
    pub fn initiate_adding_service(
        &mut self,
        instance_spec: InstanceSpec,
        constructor: impl BinaryValue,
    ) -> Result<(), ExecutionError> {
        self.0
            .child_context(Some(SUPERVISOR_INSTANCE_ID))
            .initiate_adding_service(instance_spec, constructor)
    }

    /// Initiates stopping an active service instance in the blockchain.
    ///
    /// The service is not immediately stopped; it stops if / when the block containing
    /// the stopping transaction is committed.
    pub fn initiate_stopping_service(&self, instance_id: InstanceId) -> Result<(), ExecutionError> {
        Dispatcher::initiate_stopping_service(self.0.fork, instance_id)
    }

    /// Initiates resuming previously stopped service instance in the blockchain.
    ///
    /// Provided artifact will be used in attempt to resume service. Artifact name should be equal to
    /// the artifact name of the previously stopped instance.
    /// Artifact version should be same as the `data_version` stored in the stopped service
    /// instance.
    ///
    /// This method can be used to resume modified service after successful migration.
    ///
    /// The service is not immediately activated; it activates when the block containing
    /// the activation transaction is committed.
    pub fn initiate_resuming_service(
        &mut self,
        instance_id: InstanceId,
        artifact: ArtifactId,
        params: impl BinaryValue,
    ) -> Result<(), ExecutionError> {
        let mut context = self.0.child_context(Some(SUPERVISOR_INSTANCE_ID));

        let state = DispatcherSchema::new(&*context.fork)
            .get_instance(instance_id)
            .ok_or(CoreError::IncorrectInstanceId)?;

        if state.status != Some(InstanceStatus::Stopped) {
            return Err(CoreError::ServiceNotStopped.into());
        }

        let mut spec = state.spec;
        spec.artifact = artifact;

        let runtime = context
            .dispatcher
            .runtime_by_id(spec.artifact.runtime_id)
            .ok_or(CoreError::IncorrectRuntime)?;
        runtime
            .initiate_resuming_service(context.reborrow(), &spec, params.into_bytes())
            .map_err(|mut err| {
                err.set_runtime_id(spec.artifact.runtime_id)
                    .set_call_site(|| CallSite {
                        instance_id,
                        call_type: CallType::Constructor,
                    });
                err
            })?;

        DispatcherSchema::new(&*context.fork)
            .initiate_resuming_service(instance_id, spec.artifact)
            .map_err(From::from)
    }

    /// Provides writeable access to core schema.
    pub fn writeable_core_schema(&self) -> CoreSchema<&Fork> {
        CoreSchema::new(self.0.fork)
    }

    /// Initiates data migration.
    pub fn initiate_migration(
        &self,
        new_artifact: ArtifactId,
        old_service: &str,
    ) -> Result<(), ExecutionError> {
        self.0
            .dispatcher
            .initiate_migration(self.0.fork, new_artifact, old_service)
    }

    /// Rolls back previously initiated migration.
    pub fn rollback_migration(&self, service_name: &str) -> Result<(), ExecutionError> {
        Dispatcher::rollback_migration(self.0.fork, service_name)
    }

    /// Commits the result of a previously initiated migration.
    pub fn commit_migration(
        &self,
        service_name: &str,
        migration_hash: Hash,
    ) -> Result<(), ExecutionError> {
        Dispatcher::commit_migration(self.0.fork, service_name, migration_hash)
    }

    /// Flushes a committed migration.
    pub fn flush_migration(&mut self, service_name: &str) -> Result<(), ExecutionError> {
        Dispatcher::flush_migration(self.0.fork, service_name)
    }
}

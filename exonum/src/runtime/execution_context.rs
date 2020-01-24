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
    crypto::{Hash, PublicKey},
    helpers::{Height, ValidateInput},
    merkledb::{access::Prefixed, BinaryValue, Fork},
    runtime::{
        ArtifactId, BlockchainData, CallSite, CallType, Caller, CoreError, Dispatcher,
        DispatcherSchema, ExecutionError, InstanceDescriptor, InstanceId, InstanceQuery,
        InstanceSpec, InstanceStatus, MethodId, SUPERVISOR_INSTANCE_ID,
    },
};

/// Provides the current state of the blockchain and the caller information for the call
/// which is being executed.
///
/// The call can mean a transaction call, `before_transactions` / `after_transactions` hook,
/// or the service constructor invocation.
#[derive(Debug)]
pub struct ExecutionContext<'a> {
    /// The current state of the blockchain. It includes the new, not-yet-committed, changes to
    /// the database made by the previous transactions already executed in this block.
    pub(crate) fork: &'a mut Fork,
    /// The initiator of the transaction execution.
    caller: Caller,
    /// Identifier of the service interface required for the call.
    interface_name: &'a str,
    /// ID of the executing service.
    instance: InstanceDescriptor<'a>,
    /// Hash of the currently executing transaction, or `None` for non-transaction calls.
    transaction_hash: Option<Hash>,
    /// Reference to the dispatcher.
    dispatcher: &'a Dispatcher,
    /// Depth of the call stack.
    call_stack_depth: u64,
}

impl<'a> ExecutionContext<'a> {
    /// Maximum depth of the call stack.
    pub const MAX_CALL_STACK_DEPTH: u64 = 128;

    pub(crate) fn for_transaction(
        dispatcher: &'a Dispatcher,
        fork: &'a mut Fork,
        instance: InstanceDescriptor<'a>,
        author: PublicKey,
        transaction_hash: Hash,
    ) -> Self {
        Self::new(
            dispatcher,
            fork,
            instance,
            Caller::Transaction { author },
            Some(transaction_hash),
        )
    }

    pub(crate) fn for_block_call(
        dispatcher: &'a Dispatcher,
        fork: &'a mut Fork,
        instance: InstanceDescriptor<'a>,
    ) -> Self {
        Self::new(dispatcher, fork, instance, Caller::Blockchain, None)
    }

    fn new(
        dispatcher: &'a Dispatcher,
        fork: &'a mut Fork,
        instance: InstanceDescriptor<'a>,
        caller: Caller,
        transaction_hash: Option<Hash>,
    ) -> Self {
        Self {
            dispatcher,
            fork,
            instance,
            caller,
            transaction_hash,
            interface_name: "",
            call_stack_depth: 0,
        }
    }

    /// Returns the hash of the currently executing transaction, or `None` for non-transaction
    /// root calls (e.g., `before_transactions` / `after_transactions` service hooks).
    pub fn transaction_hash(&self) -> Option<Hash> {
        self.transaction_hash
    }

    /// Provides access to blockchain data.
    pub fn data(&self) -> BlockchainData<'a, &Fork> {
        BlockchainData::new(self.fork, self.instance)
    }

    /// Provides access to the data of the executing service.
    pub fn service_data(&self) -> Prefixed<'a, &Fork> {
        self.data().for_executing_service()
    }

    /// Returns the authorization information about this call.
    pub fn caller(&self) -> &Caller {
        &self.caller
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

    /// Returns an identifier of the service interface required for the call.
    /// This identifier is always empty for the primary service interface.
    ///
    /// # Stability
    ///
    /// This getter is a part of an unfinished "interfaces" feature. It is exempt
    /// from semantic versioning and will be replaced in the future releases.
    pub fn interface_name(&self) -> &str {
        self.interface_name
    }

    /// Returns extensions required for the Supervisor service implementation.
    ///
    /// Make sure that this method invoked by the instance with the [`SUPERVISOR_INSTANCE_ID`]
    /// identifier; the call will panic otherwise.
    ///
    /// [`SUPERVISOR_INSTANCE_ID`]: constant.SUPERVISOR_INSTANCE_ID.html
    #[doc(hidden)]
    pub fn supervisor_extensions(&mut self) -> SupervisorExtensions<'_> {
        if self.instance.id != SUPERVISOR_INSTANCE_ID {
            panic!("`supervisor_extensions` called within a non-supervisor service");
        }
        SupervisorExtensions(self.reborrow(self.instance))
    }

    /// Initiates adding a new service instance to the blockchain. The created service is not active
    /// (i.e., does not process transactions or the `after_transactions` hook)
    /// until the block built on top of the provided `fork` is committed.
    ///
    /// This method should be called for the exact context passed to the runtime.
    pub(crate) fn initiate_adding_service(
        &mut self,
        spec: InstanceSpec,
        constructor: impl BinaryValue,
    ) -> Result<(), ExecutionError> {
        // TODO: revise dispatcher integrity checks [ECR-3743]
        debug_assert!(spec.validate().is_ok(), "{:?}", spec.validate());
        let runtime = self
            .dispatcher
            .runtime_by_id(spec.artifact.runtime_id)
            .ok_or(CoreError::IncorrectRuntime)?;

        let context = self.reborrow(spec.as_descriptor());
        runtime
            .initiate_adding_service(context, &spec.artifact, constructor.into_bytes())
            .map_err(|mut err| {
                err.set_runtime_id(spec.artifact.runtime_id)
                    .set_call_site(|| CallSite::new(spec.id, CallType::Constructor));
                err
            })?;

        // Add a service instance to the dispatcher schema.
        DispatcherSchema::new(&*self.fork)
            .initiate_adding_service(spec)
            .map_err(From::from)
    }

    /// Re-borrows an execution context with the given instance descriptor.
    fn reborrow<'s>(&'s mut self, instance: InstanceDescriptor<'s>) -> ExecutionContext<'s> {
        ExecutionContext {
            fork: &mut *self.fork,
            caller: self.caller.clone(),
            transaction_hash: self.transaction_hash,
            instance,
            interface_name: self.interface_name,
            dispatcher: self.dispatcher,
            call_stack_depth: self.call_stack_depth,
        }
    }

    /// Creates context for the `make_child_call` invocation.
    ///
    /// `fallthrough_auth` defines the rules of the caller authority for child calls:
    ///
    /// - `true` means that caller does not authorize the request; caller field for child calls
    ///   will not be changed.
    /// - `false` value means that caller authorizes themselves as initiator of child call;
    ///   caller field will be changed to the initiator of this call.
    fn child_context<'s>(
        &'s mut self,
        interface_name: &'s str,
        instance: InstanceDescriptor<'s>,
        fallthrough_auth: bool,
    ) -> ExecutionContext<'s> {
        let caller = if fallthrough_auth {
            self.caller.clone()
        } else {
            Caller::Service {
                instance_id: self.instance.id,
            }
        };

        ExecutionContext {
            caller,
            transaction_hash: self.transaction_hash,
            dispatcher: self.dispatcher,
            instance,
            fork: &mut *self.fork,
            interface_name,
            call_stack_depth: self.call_stack_depth + 1,
        }
    }
}

/// Collection of unstable execution context features.
#[doc(hidden)]
pub trait ExecutionContextUnstable {
    /// Invokes the interface method of the instance with the specified ID.
    ///
    /// See explanation about [`fallthrough_auth`](struct.ExecutionContext.html#child_context).
    fn make_child_call<'q>(
        &mut self,
        called_instance: impl Into<InstanceQuery<'q>>,
        interface_name: &str,
        method_id: MethodId,
        arguments: &[u8],
        fallthrough_auth: bool,
    ) -> Result<(), ExecutionError>;
}

impl ExecutionContextUnstable for ExecutionContext<'_> {
    fn make_child_call<'q>(
        &mut self,
        called_instance: impl Into<InstanceQuery<'q>>,
        interface_name: &str,
        method_id: MethodId,
        arguments: &[u8],
        fallthrough_auth: bool,
    ) -> Result<(), ExecutionError> {
        if self.call_stack_depth + 1 >= Self::MAX_CALL_STACK_DEPTH {
            let err = CoreError::stack_overflow(Self::MAX_CALL_STACK_DEPTH);
            return Err(err);
        }

        let descriptor = self
            .dispatcher
            .get_service(called_instance)
            .ok_or(CoreError::IncorrectInstanceId)?;

        let (runtime_id, runtime) = self
            .dispatcher
            .runtime_for_service(descriptor.id)
            .ok_or(CoreError::IncorrectRuntime)?;

        let context = self.child_context(interface_name, descriptor, fallthrough_auth);
        runtime
            .execute(context, method_id, arguments)
            .map_err(|mut err| {
                err.set_runtime_id(runtime_id).set_call_site(|| {
                    CallSite::new(
                        descriptor.id,
                        CallType::Method {
                            interface: interface_name.to_owned(),
                            id: method_id,
                        },
                    )
                });
                err
            })
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
            .child_context("", self.0.instance, false)
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
        let state = DispatcherSchema::new(&*self.0.fork)
            .get_instance(instance_id)
            .ok_or(CoreError::IncorrectInstanceId)?;

        if state.status != Some(InstanceStatus::Stopped) {
            return Err(CoreError::ServiceNotStopped.into());
        }

        let mut spec = state.spec;
        spec.artifact = artifact;

        let runtime = self
            .0
            .dispatcher
            .runtime_by_id(spec.artifact.runtime_id)
            .ok_or(CoreError::IncorrectRuntime)?;

        runtime
            .initiate_resuming_service(
                self.0.child_context("", spec.as_descriptor(), false),
                &spec.artifact,
                params.into_bytes(),
            )
            .map_err(|mut err| {
                err.set_runtime_id(spec.artifact.runtime_id)
                    .set_call_site(|| CallSite::new(instance_id, CallType::Constructor));
                err
            })?;

        DispatcherSchema::new(&*self.0.fork)
            .initiate_resuming_service(instance_id, spec.artifact.clone())
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

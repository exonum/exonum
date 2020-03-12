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
        migrations::MigrationType, ArtifactId, BlockchainData, CallSite, CallType, Caller,
        CoreError, Dispatcher, DispatcherSchema, ExecutionError, ExecutionFail, InstanceDescriptor,
        InstanceId, InstanceQuery, InstanceSpec, MethodId, RuntimeFeature, SUPERVISOR_INSTANCE_ID,
    },
};

const ACCESS_ERROR_STR: &str = "An attempt to access blockchain data after execution error.";

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
    instance: InstanceDescriptor,
    /// Hash of the currently executing transaction, or `None` for non-transaction calls.
    transaction_hash: Option<Hash>,
    /// Reference to the dispatcher.
    dispatcher: &'a Dispatcher,
    /// Depth of the call stack.
    call_stack_depth: u64,
    /// Flag indicating an error occurred during the child call.
    has_child_call_error: &'a mut bool,
}

impl<'a> ExecutionContext<'a> {
    /// Maximum depth of the call stack.
    pub const MAX_CALL_STACK_DEPTH: u64 = 128;

    pub(crate) fn for_transaction(
        dispatcher: &'a Dispatcher,
        fork: &'a mut Fork,
        has_child_call_error: &'a mut bool,
        instance: InstanceDescriptor,
        author: PublicKey,
        transaction_hash: Hash,
    ) -> Self {
        Self::new(
            dispatcher,
            fork,
            has_child_call_error,
            instance,
            Caller::Transaction { author },
            Some(transaction_hash),
        )
    }

    pub(crate) fn for_block_call(
        dispatcher: &'a Dispatcher,
        fork: &'a mut Fork,
        has_child_call_error: &'a mut bool,
        instance: InstanceDescriptor,
    ) -> Self {
        Self::new(
            dispatcher,
            fork,
            has_child_call_error,
            instance,
            Caller::Blockchain,
            None,
        )
    }

    fn new(
        dispatcher: &'a Dispatcher,
        fork: &'a mut Fork,
        has_child_call_error: &'a mut bool,
        instance: InstanceDescriptor,
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
            has_child_call_error,
        }
    }

    /// Returns the hash of the currently executing transaction, or `None` for non-transaction
    /// root calls (e.g., `before_transactions` / `after_transactions` service hooks).
    pub fn transaction_hash(&self) -> Option<Hash> {
        self.transaction_hash
    }

    /// Provides access to blockchain data.
    pub fn data(&self) -> BlockchainData<&Fork> {
        if *self.has_child_call_error {
            panic!(ACCESS_ERROR_STR);
        }

        BlockchainData::new(self.fork, &self.instance.name)
    }

    /// Provides access to the data of the executing service.
    pub fn service_data(&self) -> Prefixed<&Fork> {
        self.data().for_executing_service()
    }

    /// Returns the authorization information about this call.
    pub fn caller(&self) -> &Caller {
        &self.caller
    }

    /// Returns a descriptor of the executing service instance.
    pub fn instance(&self) -> &InstanceDescriptor {
        &self.instance
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
        SupervisorExtensions(self.reborrow(self.instance.clone()))
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
                self.should_rollback();
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
    fn reborrow(&mut self, instance: InstanceDescriptor) -> ExecutionContext<'_> {
        if *self.has_child_call_error {
            panic!(ACCESS_ERROR_STR);
        }

        ExecutionContext {
            fork: &mut *self.fork,
            caller: self.caller.clone(),
            transaction_hash: self.transaction_hash,
            instance,
            interface_name: self.interface_name,
            dispatcher: self.dispatcher,
            call_stack_depth: self.call_stack_depth,
            has_child_call_error: self.has_child_call_error,
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
        instance: InstanceDescriptor,
        fallthrough_auth: bool,
    ) -> ExecutionContext<'s> {
        if *self.has_child_call_error {
            panic!(ACCESS_ERROR_STR);
        }

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
            has_child_call_error: self.has_child_call_error,
        }
    }

    /// Sets the flag that the fork should rollback after this execution.
    pub(crate) fn should_rollback(&mut self) {
        *self.has_child_call_error = true;
    }
}

/// Collection of unstable execution context features.
///
/// # Safety
///
/// Errors that occur after making nested calls should be bubbled up to the upper level.
///
/// If an error has occurred in a nested call, but the returned result of the topmost
/// call is `Ok(())`, the latter will be coerced to an error `CoreError::InvalidCall`
/// and recorded as such in the blockchain. Accessing storage after an error in a
/// nested call will result in a panic.
///
/// Nested calls is a part of an unfinished "interfaces" feature. It is exempt
/// from semantic versioning and will be replaced in the future releases.
#[doc(hidden)]
pub trait ExecutionContextUnstable {
    /// Invokes the interface method of the instance with the specified ID.
    ///
    /// See explanation about [`fallthrough_auth`](struct.ExecutionContext.html#child_context).
    ///
    /// # Return value
    ///
    /// If this method returns an error, the error should bubble up to the top level.
    /// In this case do not access the blockchain data through this context methods, this will
    /// lead to panic.
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
        let instance_id = descriptor.id;
        let (runtime_id, runtime) = self
            .dispatcher
            .runtime_for_service(instance_id)
            .ok_or(CoreError::IncorrectRuntime)?;

        let context = self.child_context(interface_name, descriptor, fallthrough_auth);
        runtime
            .execute(context, method_id, arguments)
            .map_err(|mut err| {
                self.should_rollback();
                err.set_runtime_id(runtime_id).set_call_site(|| {
                    CallSite::new(
                        instance_id,
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

impl SupervisorExtensions<'_> {
    /// Marks an artifact as *committed*, i.e., one which service instances can be deployed from.
    ///
    /// If / when a block with this instruction is accepted, artifact deployment becomes
    /// a requirement for all nodes in the network. A node that did not successfully
    /// deploy the artifact previously blocks until the artifact is deployed successfully.
    /// If a node cannot deploy the artifact, it panics.
    pub fn start_artifact_registration(&self, artifact: &ArtifactId, spec: Vec<u8>) {
        Dispatcher::commit_artifact(self.0.fork, artifact, spec);
    }

    /// Unloads the specified artifact, making it unavailable for service deployment and other
    /// operations.
    ///
    /// Like other operations concerning services or artifacts, the artifact will be unloaded
    /// only if / when the block with this instruction is committed.
    ///
    /// # Return value
    ///
    /// If the artifact cannot be unloaded, an error is returned.
    pub fn unload_artifact(&self, artifact: &ArtifactId) -> Result<(), ExecutionError> {
        Dispatcher::unload_artifact(self.0.fork, artifact)
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
            .child_context("", self.0.instance.clone(), false)
            .initiate_adding_service(instance_spec, constructor)
    }

    /// Initiates stopping an active or frozen service instance.
    ///
    /// The service is not immediately stopped; it stops if / when the block containing
    /// the stopping transaction is committed.
    pub fn initiate_stopping_service(&self, instance_id: InstanceId) -> Result<(), ExecutionError> {
        Dispatcher::initiate_stopping_service(self.0.fork, instance_id)
    }

    /// Initiates freezing an active service instance.
    ///
    /// The service is not immediately frozen; it freezes if / when the block containing
    /// the stopping transaction is committed.
    ///
    /// Note that this method **cannot** be used to transition service to frozen
    /// from the stopped state; this transition is not supported as of now.
    pub fn initiate_freezing_service(&self, instance_id: InstanceId) -> Result<(), ExecutionError> {
        self.0
            .dispatcher
            .initiate_freezing_service(self.0.fork, instance_id)
    }

    /// Initiates resuming previously stopped service instance in the blockchain.
    ///
    /// This method can be used to resume modified service after successful migration.
    ///
    /// The service is not immediately activated; it activates when the block containing
    /// the activation transaction is committed.
    pub fn initiate_resuming_service(
        &mut self,
        instance_id: InstanceId,
        params: impl BinaryValue,
    ) -> Result<(), ExecutionError> {
        let state = DispatcherSchema::new(&*self.0.fork)
            .get_instance(instance_id)
            .ok_or(CoreError::IncorrectInstanceId)?;

        // Check that the service can be resumed.
        if let Some(data_version) = state.data_version {
            let msg = format!(
                "Cannot resume service `{}` because its data version ({}) does not match \
                 the associated artifact `{}`. To solve, associate the service with the newer \
                 artifact revision, for example, via fast-forward migration.",
                state.spec.as_descriptor(),
                data_version,
                state.spec.artifact
            );
            return Err(CoreError::CannotResumeService.with_description(msg));
        }

        let spec = state.spec;
        DispatcherSchema::new(&*self.0.fork).initiate_resuming_service(instance_id)?;

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
                self.0.should_rollback();
                err.set_runtime_id(spec.artifact.runtime_id)
                    .set_call_site(|| CallSite::new(instance_id, CallType::Resume));
                err
            })
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
    ) -> Result<MigrationType, ExecutionError> {
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

    /// Checks if the runtime supports the specified optional feature.
    ///
    /// # Panics
    ///
    /// - Panics if the runtime with the given identifier does not exist. This will never happen
    ///   if `runtime_id` is taken from a deployed artifact.
    pub fn check_feature(&self, runtime_id: u32, feature: &RuntimeFeature) -> bool {
        self.0
            .dispatcher
            .runtime_by_id(runtime_id)
            .unwrap_or_else(|| {
                panic!("Runtime with ID {} does not exist", runtime_id);
            })
            .is_supported(feature)
    }
}

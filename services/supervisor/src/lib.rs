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

//! Supervisor is an [Exonum][exonum] service capable of the following activities:
//!
//! - Service artifact deployment;
//! - Service instances creation;
//! - Changing consensus configuration;
//! - Changing service instances configuration.
//! - Migrating service data.
//!
//! More information on the artifact/service lifecycle can be found in the documentation for
//! the Exonum [runtime module][runtime-docs].
//!
//! Supervisor service has two different operating modes: a "simple" mode and a "decentralized" mode.
//! The difference between modes is in the decision making approach:
//! - Within the decentralized mode, to deploy a service or apply a new configuration,
//!  no less than (2/3)+1 validators should reach a consensus;
//! - Within the simple mode, any decision is executed after a single validator approval.
//!
//! The simple mode can be useful if one network administrator manages all the validator nodes
//! or for testing purposes (e.g., to test service configuration with `TestKit`).
//! For a network with a low node confidence, consider using the decentralized mode.
//!
//! # Interaction
//!
//! The intended way to interact with supervisor is the REST API. To be precise, requests should
//! be sent to the one of the following endpoints: `deploy-artifact`, `propose-config` or
//! `confirm-config`. Once received, supervisor will convert the request into appropriate
//! transaction, sign it with the validator keys and broadcast for the rest of the network.
//!
//! Key point here is that user **should not** send transactions to the supervisor by himself.
//!
//! An expected format of requests for those endpoints is a serialized Protobuf message.
//!
//! To deploy an artifact, one (within the "simple" mode) or majority (within the "decentralized" mode)
//! of the nodes should receive a [`DeployRequest`] message through API.
//!
//! To request a config change, one node should receive a [`ConfigPropose`] message through API.
//! For the "simple" mode no more actions are required. For the "decentralized" mode the majority of the nodes
//! should also receive [`ConfigVote`] messages with a hash of the proposed configuration.
//! The proposal initiator that receives the original [`ConfigPropose`] message must not vote for the configuration.
//! This node votes for the configuration propose automatically.
//!
//! The operation of starting or resuming a service is treated similarly to a configuration change
//! and follows the same rules.
//!
//! ## Migrations Management
//!
//! Supervisor service provides a functionality to perform data migrations for services.
//! Request for migration is sent through private REST API and contains the name of instance
//! to migrate, end artifact version to achieve after migration, and deadline height until which
//! migration should be completed.
//!
//! ### Requirements
//!
//! The following requirements should be satisfied in order to start a migration:
//!
//! - Target service instance should exist and be stopped.
//! - End artifact for a migration should be a superior version of the artifact of target instance.
//! - New (end) version of artifact should be deployed.
//! - Service should have all the migration scripts required to migrate to the end artifact version.
//!
//! Violation of any of requirements listed above will result in a request failure without
//! actual start of migration.
//!
//! ## Migration Workflow
//!
//! Migration starts after the block with the request is committed, and performed asynchronously.
//!
//! After the local migration completion, validator nodes report the result of migration, which can
//! be either successful or unsuccessful.
//!
//! If all validators report the successful local migration result, and the resulting state hashes
//! match, migration is committed and flushed in the block, next to block with the last required
//! migration confirmation.
//!
//! In any other case (e.g. migration failure for at least one node, resulting state hash divergence,
//! lack of report at the deadline height), migration is considered failed and rolled back.
//!
//! After fixing the reason for migration failure, the migration attempt can be performed once again.
//! It will require a different deadline height though, since `MigrationRequest` objects are considered
//! unique and supervisor won't attempt to perform the same `MigrationRequest` again.
//!
//! ### Complex Migrations
//!
//! If migration contains more than one migration script (e.g. if you need to migrate service from
//! version 0.1 to version 0.3, and this will include execution of two migration scripts: 0.1 -> 0.2
//! and 0.2 -> 0.3), supervisor will perform one migration script at the time.
//!
//! After the first migration request to version 0.3, migration will be performed for version 0.2,
//! and you need to create the same migration request again (with different deadline height though).
//!
//! After the second migration request, the version will be updated to 0.3.
//!
//! To put it simply, you may need to perform the same migration request several times until every
//! step of migration is completed.
//!
//! ### Incomplete Migrations
//!
//! Migrations require only the current and the last version of artifact to be deployed. If you will
//! decide to stop migration until reaching the last version (e.g. you requested migration to version
//! 0.3, but decided to go with version 0.2), you will have to deploy a corresponding artifact after
//! you migrate to version 0.2, before you'll be able to start your service.
//!
//! # HTTP API
//!
//! REST API of the service is documented in the [`api` module](api/index.html).
//!
//! [exonum]: https://github.com/exonum/exonum
//! [runtime-docs]: https://docs.rs/exonum/latest/exonum/runtime/index.html
//! [`DeployRequest`]: struct.DeployRequest.html
//! [`ConfigPropose`]: struct.ConfigPropose.html
//! [`ConfigVote`]: struct.ConfigVote.html

#![deny(
    missing_debug_implementations,
    missing_docs,
    unsafe_code,
    bare_trait_objects
)]

pub use self::{
    configure::{Configure, CONFIGURE_INTERFACE_NAME},
    errors::{ArtifactError, CommonError, ConfigurationError, MigrationError, ServiceError},
    event_state::AsyncEventState,
    migration_state::MigrationState,
    proto_structures::{
        ConfigChange, ConfigProposalWithHash, ConfigPropose, ConfigVote, DeployRequest,
        DeployResult, MigrationRequest, MigrationResult, ResumeService, ServiceConfig,
        StartService, StopService, SupervisorConfig,
    },
    schema::Schema,
    transactions::SupervisorInterface,
};

#[doc(hidden)] // Public for migration tests.
pub use self::schema::SchemaImpl;

use exonum::{
    blockchain::config::InstanceInitParams,
    runtime::{ExecutionContext, ExecutionError, InstanceId, SUPERVISOR_INSTANCE_ID},
};
use exonum_derive::*;
use exonum_merkledb::BinaryValue;
use exonum_rust_runtime::{
    api::ServiceApiBuilder, AfterCommitContext, Broadcaster, Service, ServiceFactory as _,
};

use crate::{configure::ConfigureMut, mode::Mode};

pub mod api;
pub mod mode;

mod configure;
mod errors;
mod event_state;
mod migration_state;
mod multisig;
mod proto;
mod proto_structures;
mod schema;
mod transactions;

/// Returns the `Supervisor` entity name.
pub const fn supervisor_name() -> &'static str {
    Supervisor::NAME
}

/// Error message emitted when the `Supervisor` is installed as a non-privileged service.
const NOT_SUPERVISOR_MSG: &str = "`Supervisor` is installed as a non-privileged service. \
                                  For correct operation, `Supervisor` needs to have numeric ID 0.";

/// Applies configuration changes.
/// Upon any failure, execution of this method stops and `Err(())` is returned.
fn update_configs(
    context: &mut ExecutionContext<'_>,
    changes: Vec<ConfigChange>,
) -> Result<(), ExecutionError> {
    for change in changes.into_iter() {
        match change {
            ConfigChange::Consensus(config) => {
                log::trace!("Updating consensus configuration {:?}", config);

                context
                    .supervisor_extensions()
                    .writeable_core_schema()
                    .consensus_config_entry()
                    .set(config);
            }

            ConfigChange::Service(config) => {
                log::trace!(
                    "Updating service instance configuration, instance ID is {}",
                    config.instance_id
                );

                // The service config was verified,
                // so panic on `expect` here is unlikely and means a bug in the implementation.
                context
                    .apply_config(config.instance_id, config.params.clone())
                    .map_err(|err| {
                        log::error!(
                            "An error occurred while applying service configuration. {}",
                            err
                        );
                        err
                    })?;
            }

            ConfigChange::StartService(start_service) => {
                log::trace!(
                    "Request add service with name {} from artifact {}",
                    start_service.name,
                    start_service.artifact
                );

                let id = assign_instance_id(context);
                let (instance_spec, config) = start_service.into_parts(id);

                context
                    .supervisor_extensions()
                    .initiate_adding_service(instance_spec, config)
                    .map_err(|err| {
                        log::error!("Service start request failed. {}", err);
                        err
                    })?;
            }

            ConfigChange::StopService(stop_service) => {
                let instance = context
                    .data()
                    .for_dispatcher()
                    .get_instance(stop_service.instance_id)
                    .expect(
                        "BUG: Instance with the specified ID is absent in the dispatcher schema.",
                    );

                log::trace!(
                    "Request stop service with name {} from artifact {}",
                    instance.spec.name,
                    instance.spec.artifact
                );

                context
                    .supervisor_extensions()
                    .initiate_stopping_service(stop_service.instance_id)?;
            }

            ConfigChange::ResumeService(resume_service) => {
                let instance = context
                    .data()
                    .for_dispatcher()
                    .get_instance(resume_service.instance_id)
                    .expect(
                        "BUG: Instance with the specified ID is absent in the dispatcher schema.",
                    );

                log::trace!(
                    "Request resume service with name {} from artifact {} up to {}",
                    instance.spec.name,
                    instance.spec.artifact,
                    resume_service.artifact,
                );

                context.supervisor_extensions().initiate_resuming_service(
                    resume_service.instance_id,
                    resume_service.artifact,
                    resume_service.params,
                )?;
            }
        }
    }
    Ok(())
}

/// Assigns the instance ID for a new service, initializing the schema `vacant_instance_id`
/// entry if needed.
fn assign_instance_id(context: &ExecutionContext<'_>) -> InstanceId {
    let mut schema = SchemaImpl::new(context.service_data());
    match schema.assign_instance_id() {
        Some(id) => id,
        None => {
            // Instance ID entry is not initialized, do it now.
            // We have to do it lazy, since dispatcher doesn't know the amount
            // of builtin instances until the genesis block is committed, and
            // `after_transactions` hook is not invoked for services at the genesis
            // block.

            // ID for the new instance is next to the highest builtin ID to avoid
            // overlap if builtin identifiers space is sparse.
            let dispatcher_schema = context.data().for_dispatcher();
            let builtin_instances = dispatcher_schema.service_instances();

            let new_instance_id = builtin_instances
                .values()
                .map(|state| state.spec.id)
                .max()
                .unwrap_or(SUPERVISOR_INSTANCE_ID)
                + 1;

            // We're going to use ID obtained above, so the vacant ID is next to it.
            let vacant_instance_id = new_instance_id + 1;
            schema.vacant_instance_id.set(vacant_instance_id);

            new_instance_id
        }
    }
}

/// Supervisor service implementation.
#[derive(Debug, Default, Clone, ServiceFactory, ServiceDispatcher)]
#[service_dispatcher(implements(
    "SupervisorInterface",
    raw = "Configure<Params = SupervisorConfig>"
))]
#[service_factory(proto_sources = "proto", artifact_name = "exonum-supervisor")]
pub struct Supervisor;

impl Supervisor {
    /// Name of the supervisor service.
    pub const NAME: &'static str = "supervisor";

    /// Creates a configuration for a simple `Supervisor`.
    pub fn simple_config() -> SupervisorConfig {
        SupervisorConfig { mode: Mode::Simple }
    }

    /// Creates a configuration for a decentralized `Supervisor`.
    pub fn decentralized_config() -> SupervisorConfig {
        SupervisorConfig {
            mode: Mode::Decentralized,
        }
    }

    /// Creates an `InstanceCollection` for builtin `Supervisor` instance with
    /// simple configuration.
    pub fn simple() -> InstanceInitParams {
        Self::builtin_instance(Self::simple_config())
    }

    /// Creates an `InstanceCollection` for builtin `Supervisor` instance with
    /// decentralized configuration.
    pub fn decentralized() -> InstanceInitParams {
        Self::builtin_instance(Self::decentralized_config())
    }

    /// Creates an `InstanceCollection` with builtin `Supervisor` instance given the
    /// configuration.
    pub fn builtin_instance(config: SupervisorConfig) -> InstanceInitParams {
        Supervisor
            .artifact_id()
            .into_default_instance(SUPERVISOR_INSTANCE_ID, Self::NAME)
            .with_constructor(config)
    }
}

impl Service for Supervisor {
    fn initialize(
        &self,
        context: ExecutionContext<'_>,
        params: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        use std::borrow::Cow;

        // Load configuration from bytes and store it.
        // Since `Supervisor` is expected to be created at the start of the blockchain, invalid config
        // will cause genesis block creation to fail, and thus blockchain won't start.
        let config = SupervisorConfig::from_bytes(Cow::from(&params))
            .map_err(|_| ConfigurationError::InvalidConfig)?;

        let mut schema = SchemaImpl::new(context.service_data());
        schema.public.configuration.set(config);

        Ok(())
    }

    fn before_transactions(&self, mut context: ExecutionContext<'_>) -> Result<(), ExecutionError> {
        self.remove_outdated_deployments(&context);
        self.remove_outdated_config_proposal(&context);
        self.flush_completed_migrations(&mut context)?;
        self.remove_outdated_migrations(&mut context)?;
        Ok(())
    }

    fn after_transactions(&self, mut context: ExecutionContext<'_>) -> Result<(), ExecutionError> {
        let mut schema = SchemaImpl::new(context.service_data());
        let configuration = schema.supervisor_config();
        let core_schema = context.data().for_core();
        let next_height = core_schema.next_height();
        let validator_count = core_schema.consensus_config().validator_keys.len();

        // Check if we should apply a new config.
        let entry = schema.public.pending_proposal.get();
        if let Some(entry) = entry {
            if entry.config_propose.actual_from == next_height {
                // Config should be applied at the next height.
                if configuration.mode.config_approved(
                    &entry.propose_hash,
                    &schema.config_confirms,
                    validator_count,
                ) {
                    log::info!(
                        "New configuration has been accepted: {:?}",
                        entry.config_propose
                    );

                    // Remove config from proposals.
                    // If the config update will fail, this entry will be restored due to rollback.
                    // However, it won't be actual anymore and will be removed at the beginning
                    // of the next height (within `before_transactions` hook).
                    schema.public.pending_proposal.remove();
                    drop(schema);

                    // Perform the application of configs.
                    update_configs(&mut context, entry.config_propose.changes)?;
                }
            }
        }
        Ok(())
    }

    /// Sends confirmation transaction for unconfirmed deployment requests.
    fn after_commit(&self, mut context: AfterCommitContext<'_>) {
        self.process_unconfirmed_deployments(&mut context);
        self.process_incomplete_migrations(&mut context);
    }

    fn wire_api(&self, builder: &mut ServiceApiBuilder) {
        api::wire(builder)
    }
}

impl Supervisor {
    /// Removes deployments for which deadline height is already exceeded.
    fn remove_outdated_deployments(&self, context: &ExecutionContext<'_>) {
        let mut schema = SchemaImpl::new(context.service_data());
        let core_schema = context.data().for_core();
        let height = core_schema.height();

        // Collect pending deploy requests for which deadline was exceeded.
        let requests_to_remove = schema
            .pending_deployments
            .values()
            .filter(|request| request.deadline_height <= height)
            .collect::<Vec<_>>();

        for request in requests_to_remove {
            schema.pending_deployments.remove(&request.artifact);
            if let Some(AsyncEventState::Pending) = schema.deploy_states.get(&request) {
                // If state is marked as pending, change it to failed as well.
                schema.deploy_states.put(&request, AsyncEventState::Timeout);
            }
            log::trace!("Removed outdated deployment request {:?}", request);
        }
    }

    /// Removes pending config proposal if it's outdated.
    fn remove_outdated_config_proposal(&self, context: &ExecutionContext<'_>) {
        let mut schema = SchemaImpl::new(context.service_data());
        let core_schema = context.data().for_core();
        let height = core_schema.height();

        let entry = schema.public.pending_proposal.get();
        if let Some(entry) = entry {
            if entry.config_propose.actual_from <= height {
                // Remove pending config proposal for which deadline was exceeded.
                log::trace!("Removed outdated config proposal");
                schema.public.pending_proposal.remove();
            }
        }
    }

    /// Goes through pending deployments, chooses ones that we're not confirmed by our node
    /// and starts the local deployment routine for them.
    fn process_unconfirmed_deployments(&self, context: &mut AfterCommitContext<'_>) {
        let service_key = context.service_key();

        let deployments: Vec<_> = {
            let schema = SchemaImpl::new(context.service_data());
            schema
                .pending_deployments
                .values()
                .filter(|request| {
                    if let Some(AsyncEventState::Pending) = schema.deploy_states.get(&request) {
                        // From all pending requests we are interested only in ones not
                        // confirmed by us.
                        !schema
                            .deploy_confirmations
                            .confirmed_by(&request, &service_key)
                    } else {
                        false
                    }
                })
                .collect()
        };

        for unconfirmed_request in deployments {
            let artifact = unconfirmed_request.artifact.clone();
            let spec = unconfirmed_request.spec.clone();
            let tx_sender = context.broadcaster().map(Broadcaster::into_owned);

            let mut extensions = context.supervisor_extensions().expect(NOT_SUPERVISOR_MSG);
            // We should deploy the artifact for all nodes, but send confirmations only
            // if the node is a validator.
            extensions.start_deploy(artifact, spec, move |result| {
                if let Some(tx_sender) = tx_sender {
                    log::trace!("Sending deployment result report {:?}", unconfirmed_request);
                    let confirmation = DeployResult::new(unconfirmed_request, result);
                    if let Err(e) = tx_sender.report_deploy_result((), confirmation) {
                        log::error!("Cannot send `DeployResult`: {}", e);
                    }
                }
                Ok(())
            });
        }
    }

    /// Flushes completed migrations and removes them from the list of pending.
    ///
    /// This has to be done in the block other than one in which migration was committed,
    /// so this method is invoked in `before_transactions` of the next block.
    fn flush_completed_migrations(
        &self,
        context: &mut ExecutionContext<'_>,
    ) -> Result<(), ExecutionError> {
        let mut schema = SchemaImpl::new(context.service_data());

        // Collect pending migration requests which are successfully completed.
        let finished_migrations = schema
            .migrations_to_flush
            .iter()
            .map(|(_, request)| request)
            .collect::<Vec<_>>();

        // Clear the index, since we will flush all the migrations now.
        schema.migrations_to_flush.clear();

        drop(schema);
        for request in finished_migrations {
            // Flush the migration.
            // This has to be done before the state update, so core will update the data version
            // for instance.
            context
                .supervisor_extensions()
                .flush_migration(request.service.as_ref())?;
            log::trace!("Flushed and finished migration with request {:?}", request);

            let mut schema = SchemaImpl::new(context.service_data());

            // Update the state of a migration.
            let mut state = schema.migration_state_unchecked(&request);
            let instance = transactions::get_instance_by_name(&context, request.service.as_ref())
                .expect("BUG: Migration succeed, but there is no such instance in core");
            state.update(AsyncEventState::Succeed, instance.data_version().clone());
            schema.migration_states.put(&request, state);
        }

        Ok(())
    }

    /// Rollbacks and removes migrations for which deadline height is already exceeded.
    fn remove_outdated_migrations(
        &self,
        context: &mut ExecutionContext<'_>,
    ) -> Result<(), ExecutionError> {
        let height = context.data().for_core().height();

        // Collect pending migration requests for which deadline was exceeded.
        let requests_to_remove = SchemaImpl::new(context.service_data())
            .pending_migrations
            .iter()
            .filter_map(|(_, request)| {
                if request.deadline_height <= height {
                    Some(request)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        for request in requests_to_remove {
            let mut schema = SchemaImpl::new(context.service_data());
            schema.pending_migrations.remove(&request);

            let mut state = schema.migration_state_unchecked(&request);
            if state.is_pending() {
                // If state is marked as pending, change it to failed as well.
                state.fail(AsyncEventState::Timeout);
                schema.migration_states.put(&request, state);

                // Then, rollback the migration.
                drop(schema);
                context
                    .supervisor_extensions()
                    .rollback_migration(request.service.as_ref())?;
            }
            log::trace!("Removed outdated migration request {:?}", request);
        }

        Ok(())
    }

    /// Goes through incomplete migrations, checking their statuses.
    fn process_incomplete_migrations(&self, context: &mut AfterCommitContext<'_>) {
        let service_key = context.service_key();

        // First of all, check all the new migrations and request core to start them.
        let pending_migrations: Vec<_> = {
            let schema = SchemaImpl::new(context.service_data());
            schema
                .pending_migrations
                .iter()
                .filter_map(|(_, request)| {
                    let state = schema.migration_state_unchecked(&request);

                    let confirmed_by_us = schema
                        .migration_confirmations
                        .confirmed_by(&request, &service_key);

                    // We are interested in requests that are both pending and not yet confirmed.
                    // Despite the fact that `migration_confirmations` stores only successful
                    // outcomes, receiving any failure report will immediately change the state
                    // to `Failed`.
                    // Thus if request is `pending` and there is no out signature in
                    // `migration_confirmations` index, it means than we did not send the
                    // result report, and should do it once core will provide this result.
                    if state.is_pending() && !confirmed_by_us {
                        Some(request)
                    } else {
                        None
                    }
                })
                .collect()
        };

        for request in pending_migrations {
            let local_migration_result = context
                .data()
                .for_dispatcher()
                .local_migration_result(request.service.as_ref());

            let tx_sender = context.broadcaster().map(Broadcaster::into_owned);

            if let Some(status) = local_migration_result {
                // We've got a result, broadcast it if our node is a validator.
                if let Some(tx_sender) = tx_sender {
                    let confirmation = MigrationResult { request, status };

                    if let Err(e) = tx_sender.report_migration_result((), confirmation) {
                        log::error!("Cannot send `MigrationResult`: {}", e);
                    }
                }
            }
        }
    }
}

impl Configure for Supervisor {
    type Params = SupervisorConfig;

    fn verify_config(
        &self,
        _context: ExecutionContext<'_>,
        _params: Self::Params,
    ) -> Result<(), ExecutionError> {
        // If config was decoded, it's OK.
        Ok(())
    }

    fn apply_config(
        &self,
        context: ExecutionContext<'_>,
        params: Self::Params,
    ) -> Result<(), ExecutionError> {
        let mut schema = SchemaImpl::new(context.service_data());
        schema.public.configuration.set(params);
        Ok(())
    }
}

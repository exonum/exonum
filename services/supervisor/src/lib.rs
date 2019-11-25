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

//! Supervisor is an [Exonum][exonum] service capable of the following activities:
//! - Service artifact deployment;
//! - Service instances creation;
//! - Changing consensus configuration;
//! - Changing service instances configuration.
//!
//! More information on the artifact/service lifecycle can be found at the Exonum
//! [runtime module documentation][runtime-docs].
//!
//! Supervisor service has two different operating modes: "simple" and "decentralized".
//! The difference between modes is in the decision making approach:
//! - Within decentralized mode, to deploy a service or apply a new configuration, there should
//!   be no less than (2/3)+1 validator approvals for a decision;
//! - Within simple mode, any decision will be executed after just 1 validator approval.
//!
//! Simple mode can be useful if all the validator nodes are belong to the one network
//! administrator, or for development purposes.
//! For a network with a low node confidence, consider using decentralized mode.
//!
//! **Interaction:**
//!
//! On a transaction level, every request should be signed by a validator **node**. Thus, one is
//! not able to send requests as transactions. Instead, requests should be sent via private api
//! endpoints: "deploy-artifact", "propose-config" and "confirm-config".
//!
//! Expected format of requests for those endpoints is an serialized protobuf message.
//!
//! To deploy an artifact, one (within "simple" mode) or majority (within "decentralized" mode)
//! of the nodes should receive a [`DeployRequest`] message through API.
//!
//! To request a config change, one node should receive a [`ConfigPropose`] message through API.
//! For "simple" mode no more actions required, and for "decentralized" mode majority of the nodes
//! should also receive [`ConfigVote`] message with the hash of proposed config.
//! The node that received original [`ConfigPropose`] message shouldn't vote for a configuration,
//! the proposal initiator votes for it automatically.
//!
//! Service start operation is considered a configuration change as well and follow the same rules.
//!
//! [exonum]: https://github.com/exonum/exonum
//! [runtime-docs]: https://docs.rs/exonum/0.13.0/exonum/runtime/index.html
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
    configure::{Configure, ConfigureCall, CONFIGURE_INTERFACE_NAME},
    errors::Error,
    proto_structures::{
        ConfigChange, ConfigProposalWithHash, ConfigPropose, ConfigVote, DeployConfirmation,
        DeployRequest, ServiceConfig, StartService,
    },
    schema::Schema,
    transactions::SupervisorInterface,
};

use exonum::{
    blockchain::InstanceCollection,
    crypto::Hash,
    runtime::{
        api::ServiceApiBuilder,
        rust::{AfterCommitContext, CallContext, Service, Transaction},
        BlockchainData, InstanceId, SUPERVISOR_INSTANCE_ID,
    },
};
use exonum_derive::*;
use exonum_merkledb::Snapshot;

pub mod mode;

mod api;
mod configure;
mod errors;
mod multisig;
mod proto;
mod proto_structures;
mod schema;
mod transactions;

/// Decentralized supervisor.
///
/// Within decentralized mode, both deploy requests and configuration change proposals
/// should be approved by (2/3+1) validators.
pub type DecentralizedSupervisor = Supervisor<mode::Decentralized>;

/// Simple supervisor.
///
/// Within simple mode, both deploy requests and configuration change proposals require
/// only one approval from a validator node.
pub type SimpleSupervisor = Supervisor<mode::Simple>;

/// Returns the `Supervisor` entity name.
pub const fn supervisor_name() -> &'static str {
    Supervisor::<mode::Decentralized>::NAME
}

/// Error message emitted when the `Supervisor` is installed as a non-privileged service.
const NOT_SUPERVISOR_MSG: &str = "`Supervisor` is installed as a non-privileged service. \
                                  For correct operation, `Supervisor` needs to have numeric ID 0.";

/// Applies configuration changes.
/// Upon any failure, execution of this method stops and `Err(())` is returned.
fn update_configs(context: &mut CallContext<'_>, changes: Vec<ConfigChange>) -> Result<(), ()> {
    for change in changes.into_iter() {
        match change {
            ConfigChange::Consensus(config) => {
                log::trace!("Updating consensus configuration {:?}", config);

                context
                    .writeable_core_schema()
                    .consensus_config_entry()
                    .set(config);
            }

            ConfigChange::Service(config) => {
                log::trace!(
                    "Updating service instance configuration, instance ID is {}",
                    config.instance_id
                );

                // `ConfigureCall` interface was checked during the config verifying
                // so panic on `expect` here is unlikely and means a bug in the implementation.
                context
                    .interface::<ConfigureCall<'_>>(config.instance_id)
                    .expect("Obtaining Configure interface failed")
                    .apply_config(config.params.clone())
                    .map_err(|e| {
                        log::error!(
                            "An error occurred while applying service configuration. {}",
                            e
                        );
                    })?;
            }

            ConfigChange::StartService(start_service) => {
                log::trace!(
                    "Request add service with name {:?} from artifact {:?}",
                    start_service.name,
                    start_service.artifact
                );

                let id = assign_instance_id(context);
                let (instance_spec, config) = start_service.into_parts(id);

                context
                    .start_adding_service(instance_spec, config)
                    .map_err(|e| {
                        log::error!("Service start request failed. {}", e);
                    })?;
            }
        }
    }
    Ok(())
}

/// Assigns the instance ID for a new service, initializing the schema `vacant_instance_id`
/// entry if needed.
fn assign_instance_id(context: &CallContext<'_>) -> InstanceId {
    let mut schema = Schema::new(context.service_data());
    match schema.assign_instance_id() {
        Some(id) => id,
        None => {
            // Instance ID entry is not initialized, do it now.
            // We have to do it lazy, since dispatcher doesn't know the amount
            // of builtin instances until the genesis block is committed, and
            // `before_commit` hook is not invoked for services at the genesis
            // block.

            // ID for the new instance is next to the highest builtin ID to avoid
            // overlap if builtin identifiers space is sparse.
            let dispatcher_schema = context.data().for_dispatcher();
            let builtin_instances = dispatcher_schema.running_instances();

            let new_instance_id = builtin_instances
                .values()
                .map(|spec| spec.id)
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
#[service_dispatcher(implements("transactions::SupervisorInterface"))]
#[service_factory(
    proto_sources = "proto",
    artifact_name = "exonum-supervisor",
    service_constructor = "Self::construct"
)]
pub struct Supervisor<Mode>
where
    Mode: mode::SupervisorMode,
{
    phantom: std::marker::PhantomData<Mode>,
}

impl<Mode> Supervisor<Mode>
where
    Mode: mode::SupervisorMode,
{
    /// Name of the supervisor service.
    pub const NAME: &'static str = "supervisor";

    /// Creates a new `Supervisor` service object.
    pub fn new() -> Supervisor<Mode> {
        Supervisor {
            phantom: std::marker::PhantomData::<Mode>::default(),
        }
    }

    /// Factory constructor of the `Supervisor` object, taking `&self` as argument.
    /// Required for `ServiceFactory` trait implementation.
    pub fn construct(&self) -> Box<Self> {
        Box::new(Self::new())
    }
}

impl<Mode> Service for Supervisor<Mode>
where
    Mode: mode::SupervisorMode,
{
    fn state_hash(&self, data: BlockchainData<&dyn Snapshot>) -> Vec<Hash> {
        Schema::new(data.for_executing_service()).state_hash()
    }

    fn before_commit(&self, mut context: CallContext<'_>) {
        let mut schema = Schema::new(context.service_data());
        let core_schema = context.data().for_core();
        let validator_count = core_schema.consensus_config().validator_keys.len();
        let height = core_schema.height();

        // Removes pending deploy requests for which deadline was exceeded.
        let requests_to_remove = schema
            .pending_deployments
            .values()
            .filter(|request| request.deadline_height < height)
            .collect::<Vec<_>>();

        for request in requests_to_remove {
            schema.pending_deployments.remove(&request.artifact);
            log::trace!("Removed outdated deployment request {:?}", request);
        }

        let entry = schema.pending_proposal.get();
        if let Some(entry) = entry {
            if entry.config_propose.actual_from <= height {
                // Remove pending config proposal for which deadline was exceeded.
                log::trace!("Removed outdated config proposal");
                schema.pending_proposal.remove();
            } else if entry.config_propose.actual_from == height.next() {
                // Config should be applied at the next height.
                if Mode::config_approved(
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
                    // However, it won't be actual anymore and will be removed at the next height.
                    schema.pending_proposal.remove();
                    drop(schema);

                    // Perform the application of configs.
                    let update_result = update_configs(&mut context, entry.config_propose.changes);

                    if update_result.is_err() {
                        // Panic will cause changes to be rolled back.
                        // TODO: Return error instead of panic once the signature
                        // of `before_commit` will allow it. [ECR-3811]
                        panic!("Config update failed")
                    }
                }
            }
        }
    }

    fn after_commit(&self, mut context: AfterCommitContext<'_>) {
        let schema = Schema::new(context.service_data());
        let keypair = context.service_keypair;
        let instance_id = context.instance.id;
        let core_schema = context.data().for_core();
        let is_validator = core_schema
            .validator_id(context.service_keypair.0)
            .is_some();

        // Sends confirmation transaction for unconfirmed deployment requests.
        let deployments: Vec<_> = schema
            .pending_deployments
            .values()
            .filter(|request| {
                let confirmation = DeployConfirmation::from(request.clone());
                !schema
                    .deploy_confirmations
                    .confirmed_by(&confirmation, &keypair.0)
            })
            .collect();
        drop(schema);

        for unconfirmed_request in deployments {
            let artifact = unconfirmed_request.artifact.clone();
            let spec = unconfirmed_request.spec.clone();
            let keypair = context.service_keypair.clone();
            let tx_sender = context.transaction_broadcaster();

            let mut extensions = context.supervisor_extensions().expect(NOT_SUPERVISOR_MSG);
            extensions.start_deploy(artifact, spec, move || {
                if is_validator {
                    log::trace!(
                        "Sending confirmation for deployment request {:?}",
                        unconfirmed_request
                    );

                    let transaction = DeployConfirmation::from(unconfirmed_request);
                    tx_sender
                        .broadcast_transaction(transaction.sign(instance_id, keypair.0, &keypair.1))
                        .map_err(|e| log::error!("Couldn't broadcast transaction {}", e))
                        .ok();
                }
                Ok(())
            });
        }
    }

    fn wire_api(&self, builder: &mut ServiceApiBuilder) {
        api::wire(builder)
    }
}

impl<Mode> From<Supervisor<Mode>> for InstanceCollection
where
    Mode: mode::SupervisorMode,
{
    fn from(service: Supervisor<Mode>) -> Self {
        InstanceCollection::new(service).with_instance(
            SUPERVISOR_INSTANCE_ID,
            Supervisor::<Mode>::NAME,
            Vec::default(),
        )
    }
}

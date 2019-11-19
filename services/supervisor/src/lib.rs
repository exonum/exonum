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
mod proto;
mod proto_structures;
mod schema;
mod transactions;

/// Instance identifier for first deployed service.
///
/// By analogy with the privileged ports of the network, we use a range 0..1023 of instance
/// identifiers for built-in services which can be created only during the blockchain genesis
/// block creation.
// TODO: remove [ECR-3851]
const MAX_BUILTIN_INSTANCE_ID: InstanceId = 1024;

/// Decentralized supervisor.
pub type DecentralizedSupervisor = Supervisor<mode::Decentralized>;

/// Simple supervisor.
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
            let id = Schema::new(context.service_data()).assign_instance_id();
            let (instance_spec, config) = start_service.into_parts(id);

            context.start_adding_service(instance_spec, config)
                    .map_err(|e| {
                        log::error!("Service start request failed. {}", e);
                    })?;
            }
        }
    }
    Ok(())
}

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

    pub fn new() -> Supervisor<Mode> {
        Supervisor {
            phantom: std::marker::PhantomData::<Mode>::default(),
        }
    }

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

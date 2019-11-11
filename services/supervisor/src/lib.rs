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
    blockchain::{self, InstanceCollection},
    crypto::Hash,
    helpers::validator::validator_id,
    runtime::{
        api::ServiceApiBuilder,
        rust::{AfterCommitContext, CallContext, Service, Transaction},
        InstanceDescriptor, SUPERVISOR_INSTANCE_ID,
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

/// Creates a new decentralized `Supervisor`.
pub fn decentralized_supervisor() -> Supervisor<mode::Decentralized> {
    Supervisor::<mode::Decentralized>::new()
}

/// Creates a new simple `Supervisor`.
pub fn simple_supervisor() -> Supervisor<mode::Simple> {
    Supervisor::<mode::Simple>::new()
}

/// Returns the `Supervisor` entity name.
pub const fn supervisor_name() -> &'static str {
    Supervisor::<mode::Decentralized>::NAME
}

/// Error message emitted when the `Supervisor` is installed as a non-privileged service.
const NOT_SUPERVISOR_MSG: &str = "`Supervisor` is installed as a non-privileged service. \
                                  For correct operation, `Supervisor` needs to have numeric ID 0.";

/// Applies configuration changes, isolating each of them with by using `Fork` checkpoints.
///
/// # Safety
///
/// This function should be used with extreme care. It makes the following assumptions:
///
/// - The function must be called at the end of the transaction or `before_commit` execution.
///   If the transaction errors / panics afterwards, the changes to the configs will not
///   be rolled back.
/// - No changes to the blockchain state should be introduced before the call to this function.
///   Any changes that are introduced will be committed regardless of the execution status,
///   or the status of application of any config change. This is if the execution wasn't interrupted
///   by a panic / error *before* hitting the call; if this happens, the usual rules apply.
///
/// These restrictions are the result of `Fork` not having multi-layered checkpoints.
fn update_configs(context: &mut CallContext<'_>, changes: Vec<ConfigChange>) {
    // An error while configuring one of the service instances should not affect others.
    changes.into_iter().for_each(|change| match change {
        ConfigChange::Consensus(config) => {
            log::trace!("Updating consensus configuration {:?}", config);

            let result = context.isolate(|context| {
                blockchain::Schema::new(context.fork())
                    .consensus_config_entry()
                    .set(config);
                Ok(())
            });
            assert!(result.is_ok());
        }

        ConfigChange::Service(config) => {
            log::trace!(
                "Updating service instance configuration, instance ID is {}",
                config.instance_id
            );

            let configure_result = context.isolate(|mut context| {
                context
                    .interface::<ConfigureCall<'_>>(config.instance_id)?
                    .apply_config(config.params.clone())
            });
            if let Err(e) = configure_result {
                log::error!(
                    "An error occurred while applying service configuration. {}",
                    e
                );
            }
        }
    })
}

#[derive(Debug, Default, Clone, Copy, ServiceFactory)]
#[exonum(
    proto_sources = "proto",
    artifact_name = "exonum-supervisor",
    service_constructor = "Self::construct",
    implements("transactions::SupervisorInterface")
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
        Box::new(*self)
    }
}

impl<Mode> Service for Supervisor<Mode>
where
    Mode: mode::SupervisorMode,
{
    fn state_hash(&self, descriptor: InstanceDescriptor<'_>, snapshot: &dyn Snapshot) -> Vec<Hash> {
        Schema::new(descriptor.name, snapshot).state_hash()
    }

    fn before_commit(&self, mut context: CallContext<'_>) {
        let schema = Schema::new(context.instance().name, context.fork());
        let height = blockchain::Schema::new(context.fork()).height();

        // Removes pending deploy requests for which deadline was exceeded.
        let requests_to_remove = schema
            .pending_deployments()
            .values()
            .filter(|request| request.deadline_height < height)
            .collect::<Vec<_>>();

        for request in requests_to_remove {
            schema.pending_deployments().remove(&request.artifact);
            log::trace!("Removed outdated deployment request {:?}", request);
        }

        let entry = schema.pending_proposal().get();
        if let Some(entry) = entry {
            if entry.config_propose.actual_from <= height {
                // Remove pending config proposal for which deadline was exceeded.
                log::trace!("Removed outdated config proposal");
                schema.pending_proposal().remove();
            } else if entry.config_propose.actual_from == height.next() {
                // Config should be applied at the next height.

                let config_confirms = schema.config_confirms();

                if Mode::config_approved(&entry.propose_hash, &config_confirms) {
                    log::info!(
                        "New configuration has been accepted: {:?}",
                        entry.config_propose
                    );
                    // Perform the application of configs.
                    update_configs(&mut context, entry.config_propose.changes);
                    // Remove config from proposals. Note that this step is performed even
                    // if applying one or more configs has errored / panicked.
                    let schema = Schema::new(context.instance().name, context.fork());
                    schema.pending_proposal().remove();
                }
            }
        }
    }

    fn after_commit(&self, mut context: AfterCommitContext<'_>) {
        let schema = Schema::new(context.instance.name, context.snapshot);
        let pending_deployments = schema.pending_deployments();
        let keypair = context.service_keypair;
        let instance_id = context.instance.id;
        let is_validator = validator_id(context.snapshot, context.service_keypair.0).is_some();

        // Sends confirmation transaction for unconfirmed deployment requests.
        let deployments = pending_deployments.values().filter(|request| {
            let confirmation = DeployConfirmation::from(request.clone());
            !schema
                .deploy_confirmations()
                .confirmed_by(&confirmation, &keypair.0)
        });

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

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

#[macro_use]
extern crate exonum_derive;
#[macro_use]
extern crate exonum;
#[macro_use]
extern crate log;

pub use self::{
    errors::Error,
    proto_structures::{
        ConfigProposalWithHash, ConfigPropose, ConfigVote, DeployConfirmation, DeployRequest,
        StartService,
    },
    schema::Schema,
};

use exonum::{
    blockchain::{self, InstanceCollection},
    crypto::Hash,
    helpers::byzantine_quorum,
    runtime::{
        api::ServiceApiBuilder,
        communication_channel::SupervisorAccess,
        mailbox::Action,
        rust::{AfterCommitContext, BeforeCommitContext, Service, Transaction},
        InstanceDescriptor, SUPERVISOR_INSTANCE_ID, SUPERVISOR_INSTANCE_NAME,
    },
};
use exonum_merkledb::Snapshot;

mod api;
mod errors;
mod proto;
mod proto_structures;
mod schema;
mod transactions;

#[derive(Debug, ServiceFactory)]
#[exonum(
    proto_sources = "proto",
    artifact_name = "exonum-supervisor",
    implements("transactions::SupervisorInterface")
)]
pub struct Supervisor;

impl Service for Supervisor {
    fn state_hash(&self, descriptor: InstanceDescriptor, snapshot: &dyn Snapshot) -> Vec<Hash> {
        Schema::new(descriptor.name, snapshot).state_hash()
    }

    fn wire_api(&self, builder: &mut ServiceApiBuilder) {
        api::wire(builder)
    }

    fn before_commit(&self, context: BeforeCommitContext) {
        let schema = Schema::new(context.instance.name, context.fork);
        let height = blockchain::Schema::new(context.fork).height();

        // Removes pending deploy requests for which deadline was exceeded.
        let requests_to_remove = schema
            .pending_deployments()
            .values()
            .filter(|request| request.deadline_height < height)
            .collect::<Vec<_>>();

        for request in requests_to_remove {
            schema.pending_deployments().remove(&request.artifact);

            trace!("Removed outdated deployment request {:?}", request);
        }

        let entry = schema.pending_proposal().get();
        if let Some(entry) = entry {
            if entry.config_propose.actual_from <= height {
                // Remove pending config proposal for which deadline was exceeded.
                trace!("Removed outdated config proposal");
                schema.pending_proposal().remove();
            } else {
                let config_confirms = schema.config_confirms();
                let confirmations = config_confirms.confirmations(&entry.propose_hash);
                let validators = config_confirms.validators_amount();

                // Apply pending config in case 2/3+1 validators voted for it.
                if confirmations >= byzantine_quorum(validators) {
                    info!(
                        "New configuration has been accepted: {:?}",
                        entry.config_propose
                    );
                    // Perform the application of configs.
                    let communication_channel =
                        context.communication_channel().supervisor_interface(self);

                    communication_channel
                        .update_config(context.instance.id, entry.config_propose.changes);
                    // Remove config from proposals.
                    schema.pending_proposal().remove();
                }
            }
        }
    }

    fn after_commit(&self, context: AfterCommitContext) {
        let schema = Schema::new(context.instance.name, context.snapshot);
        let pending_deployments = schema.pending_deployments();

        // Sends confirmation transaction for unconfirmed deployment requests.
        pending_deployments
            .values()
            .filter(|request| {
                let confirmation = DeployConfirmation::from(request.clone());
                !schema
                    .deploy_confirmations()
                    .confirmed_by(&confirmation, &context.service_keypair.0)
            })
            .for_each(|unconfirmed_request| {
                let artifact = unconfirmed_request.artifact.clone();
                let spec = unconfirmed_request.spec.clone();
                // A callback that will broadcast the `ArtifactDeployConfirmation` transaction
                // if the request for deployment completes successfully and node is validator.
                let and_then = {
                    let tx_sender = context.transaction_broadcaster();
                    let keypair = context.service_keypair.clone();
                    let instance_id = context.instance.id;
                    let is_validator = context.validator_id().is_some();
                    move || {
                        if is_validator {
                            trace!(
                                "Sent confirmation for deployment request {:?}",
                                unconfirmed_request
                            );

                            let transaction = DeployConfirmation::from(unconfirmed_request);
                            tx_sender
                                .broadcast_transaction(transaction.sign(
                                    instance_id,
                                    keypair.0,
                                    &keypair.1,
                                ))
                                .map_err(|e| error!("Couldn't broadcast transaction {}.", e))
                                .ok();
                        }
                    }
                };
                // TODO Rewrite on async await syntax. [ECR-3222]
                let action = Action::StartDeploy { artifact, spec };
                let communication_channel =
                    context.communication_channel().supervisor_interface(self);

                communication_channel.request_action(action, Some(Box::new(and_then)));
            })
    }
}

impl SupervisorAccess for Supervisor {}

impl From<Supervisor> for InstanceCollection {
    fn from(service: Supervisor) -> Self {
        InstanceCollection::new(service).with_instance(
            SUPERVISOR_INSTANCE_ID,
            SUPERVISOR_INSTANCE_NAME,
            Vec::default(),
        )
    }
}

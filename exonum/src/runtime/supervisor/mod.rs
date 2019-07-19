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
    errors::Error,
    proto::{DeployConfirmation, DeployRequest, StartService},
    schema::Schema,
};

use exonum_merkledb::Snapshot;

use crate::{
    api::ServiceApiBuilder,
    blockchain,
    crypto::Hash,
    runtime::{
        rust::{AfterCommitContext, Service, ServiceDescriptor, Transaction, TransactionContext},
        ServiceInstanceId,
    },
};

mod api;
mod errors;
pub mod multisig;
mod proto;
mod schema;
mod transactions;

#[derive(Debug, ServiceFactory)]
#[exonum(
    crate = "crate",
    proto_sources = "proto::schema",
    artifact_name = "exonum-supervisor"
)]
pub struct Supervisor;

impl Service for Supervisor {
    fn state_hash(&self, descriptor: ServiceDescriptor, snapshot: &dyn Snapshot) -> Vec<Hash> {
        Schema::new(descriptor.service_name(), snapshot).state_hash()
    }

    fn wire_api(&self, descriptor: ServiceDescriptor, builder: &mut ServiceApiBuilder) {
        api::wire(descriptor, builder)
    }

    fn before_commit(&self, context: TransactionContext) {
        let schema = Schema::new(context.service_name(), context.fork());
        let height = blockchain::Schema::new(context.fork()).height();

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
    }

    fn after_commit(&self, context: AfterCommitContext) {
        if context.validator_id().is_none() {
            return;
        }

        let schema = Schema::new(context.service_name(), context.snapshot());
        let pending_deployments = schema.pending_deployments();

        // Sends confirmation transaction for unconfirmed deployment requests.
        pending_deployments
            .values()
            .filter(|request| {
                let confirmation = DeployConfirmation::from(request.clone());
                !schema
                    .deploy_confirmations()
                    .confirmed_by(&confirmation, context.public_key())
            })
            .for_each(|unconfirmed_request| {
                let artifact = unconfirmed_request.artifact.clone();
                let spec = unconfirmed_request.spec.clone();
                // A callback that will broadcast the `ArtifactDeployConfirmation` transaction
                // if the request for deployment completes successfully.
                let and_then = {
                    let tx_sender = context.transaction_broadcaster();
                    let keypair = (*context.public_key(), context.secret_key().clone());
                    let service_id = context.service_id();
                    move || {
                        trace!(
                            "Sent confirmation for deployment request {:?}",
                            unconfirmed_request
                        );

                        let transaction = DeployConfirmation::from(unconfirmed_request);
                        tx_sender
                            .broadcast_transaction(
                                transaction.sign(service_id, keypair.0, &keypair.1),
                            )
                            .map_err(|e| error!("Couldn't broadcast transaction {}.", e))
                            .ok();
                    }
                };
                // TODO Rewrite on async await syntax. [ECR-3222]
                context
                    .dispatcher_channel()
                    .request_deploy_artifact(artifact, spec, and_then);
            })
    }
}

impl Supervisor {
    pub const BUILTIN_ID: ServiceInstanceId = 0;
    pub const BUILTIN_NAME: &'static str = "supervisor";
}

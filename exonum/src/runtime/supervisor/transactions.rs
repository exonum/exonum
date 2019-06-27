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

use crate::{
    blockchain::{self, ExecutionResult},
    runtime::{
        dispatcher::{self, Action},
        rust::TransactionContext,
        InstanceSpec, ServiceConfig,
    },
};

use super::{DeployArtifact, Error, Schema, StartService, Supervisor};

// TODO Implement generic helper module for multisig transactions [ECR-3222]

/// Supervisor service transactions.
#[service_interface(exonum(crate = "crate"))]
pub trait Transactions {
    /// Requests artifact deploy.
    fn deploy_artifact(
        &self,
        context: TransactionContext,
        artifact: DeployArtifact,
    ) -> ExecutionResult;
    /// Requests start service.
    fn start_service(&self, context: TransactionContext, service: StartService) -> ExecutionResult;
}

impl Transactions for Supervisor {
    fn deploy_artifact(
        &self,
        mut context: TransactionContext,
        artifact: DeployArtifact,
    ) -> ExecutionResult {
        let blockchain_schema = blockchain::Schema::new(context.fork());
        let validator_keys = blockchain_schema.actual_configuration().validator_keys;
        // Verifies that transaction author is validator.
        if !validator_keys
            .iter()
            .any(|validator| validator.service_key == context.author())
        {
            return Err(Error::UnknownAuthor)?;
        }
        // Verifies that the artifact is not deployed yet.
        if dispatcher::Schema::new(context.fork())
            .artifacts()
            .contains(&artifact.id.name)
        {
            return Err(Error::AlreadyDeployed)?;
        }

        // Verifies that we doesn't reach deadline height.
        if artifact.deadline_height > blockchain_schema.height() {
            return Err(Error::DeployDeadline)?;
        }

        let confirmations = Schema::new(context.service_name(), context.fork())
            .confirm_pending_artifact(&artifact, context.author());
        if confirmations == validator_keys.len() {
            debug!("Register artifact {:?}", artifact.id);
            // We have enough confirmations to register the deployed artifact in the dispatcher,
            // if this action fails this transaction will be canceled.
            context.dispatch_action(Action::RegisterArtifact {
                artifact: artifact.id,
            });
        } else {
            // Verifies that we can deploy an artifact, if this action fails,
            // this transaction will be canceled.
            context.dispatch_action(Action::DeployArtifact {
                artifact: artifact.id,
            });
        }

        Ok(())
    }

    fn start_service(
        &self,
        mut context: TransactionContext,
        service: StartService,
    ) -> ExecutionResult {
        let blockchain_schema = blockchain::Schema::new(context.fork());
        let dispatcher_schema = dispatcher::Schema::new(context.fork());

        let validator_keys = blockchain_schema.actual_configuration().validator_keys;
        // Verifies that transaction author is validator.
        if !validator_keys
            .iter()
            .any(|validator| validator.service_key == context.author())
        {
            return Err(Error::UnknownAuthor)?;
        }
        // Verifies that the instance name does not exist.
        if dispatcher_schema
            .service_instances()
            .contains(&service.name)
        {
            return Err(Error::InstanceExists)?;
        }

        let confirmations = Schema::new(context.service_name(), context.fork())
            .confirm_pending_instance(&service, context.author());
        if confirmations == validator_keys.len() {
            // Assigns identifier for the new service instance.
            let spec = InstanceSpec {
                artifact: service.artifact,
                id: dispatcher_schema.vacant_instance_id(),
                name: service.name,
            };
            debug!("Start service with spec {:?}", spec);
            // We have enough confirmations to start a new service instance,
            // if this action fails this transaction will be canceled.
            context.dispatch_action(Action::StartService {
                spec,
                config: ServiceConfig {
                    data: service.config,
                },
            })
        }

        Ok(())
    }
}

impl_service_dispatcher!(Supervisor, Transactions);

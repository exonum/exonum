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
        InstanceSpec,
    },
};

use super::{DeployArtifact, Error, Schema, StartService, Supervisor};

// TODO Implement generic helper module for multisig transactions [ECR-3222]

#[service_interface(exonum(crate = "crate"))]
/// Supervisor service transactions.
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
        deploy: DeployArtifact,
    ) -> ExecutionResult {
        let blockchain_schema = blockchain::Schema::new(context.fork());
        // Verifies that we doesn't reach deadline height.
        if deploy.deadline_height < blockchain_schema.height() {
            return Err(Error::DeadlineExceeded)?;
        }
        let mut pending_artifacts =
            Schema::new(context.service_name(), context.fork()).pending_artifacts();
        let author = context.author();
        // Verifies that transaction author is validator.
        pending_artifacts
            .validator_id(author)
            .ok_or(Error::UnknownAuthor)?;
        // Verifies that the artifact is not deployed yet.
        if dispatcher::Schema::new(context.fork())
            .artifacts()
            .contains(&deploy.artifact.name)
        {
            return Err(Error::AlreadyDeployed)?;
        }

        let confirmations = pending_artifacts.confirm(&deploy, author);
        if confirmations == pending_artifacts.validators_len() {
            trace!("Request register artifact {:?}", deploy.artifact);
            // We have enough confirmations to register the deployed artifact in the dispatcher,
            // if this action fails this transaction will be canceled.
            context.dispatch_action(Action::RegisterArtifact {
                artifact: deploy.artifact,
                spec: deploy.spec,
            });
        } else {
            trace!("Request deploy artifact {:?}", deploy.artifact);
            // Verifies that we can deploy an artifact, if this action fails,
            // this transaction will be canceled.
            context.dispatch_action(Action::DeployArtifact {
                artifact: deploy.artifact,
                spec: deploy.spec,
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
        // Verifies that we doesn't reach deadline height.
        if service.deadline_height < blockchain_schema.height() {
            return Err(Error::DeadlineExceeded)?;
        }
        let mut pending_instances =
            Schema::new(context.service_name(), context.fork()).pending_instances();
        let author = context.author();
        // Verifies that transaction author is validator.
        pending_instances
            .validator_id(author)
            .ok_or(Error::UnknownAuthor)?;
        // Verifies that the instance name does not exist.
        if dispatcher_schema
            .service_instances()
            .contains(&service.name)
        {
            return Err(Error::InstanceExists)?;
        }

        let confirmations = pending_instances.confirm(&service, author);
        if confirmations == pending_instances.validators_len() {
            // Assigns identifier for the new service instance.
            let spec = InstanceSpec {
                artifact: service.artifact,
                id: dispatcher_schema.vacant_instance_id(),
                name: service.name,
            };
            trace!("Request start service with spec {:?}", spec);
            // We have enough confirmations to start a new service instance,
            // if this action fails this transaction will be canceled.
            context.dispatch_action(Action::StartService {
                spec,
                config: service.config,
            })
        }

        Ok(())
    }
}

impl_service_dispatcher!(Supervisor, Transactions);

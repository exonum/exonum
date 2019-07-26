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
    blockchain,
    helpers::ValidateInput,
    runtime::{
        dispatcher::{self, Action},
        rust::TransactionContext,
        ExecutionError, InstanceSpec,
    },
};

use super::{DeployConfirmation, DeployRequest, Error, Schema, StartService, Supervisor};

/// Supervisor service transactions.
#[exonum_service(crate = "crate", dispatcher = "super::Supervisor")]
pub trait Transactions {
    /// Requests artifact deploy.
    ///
    /// This request should be sent by the each of validators.
    /// After that, the supervisor will try to deploy the artifact, and if this procedure
    /// will be successful it will send `confirm_artifact_deploy` transaction.
    fn request_artifact_deploy(
        &self,
        context: TransactionContext,
        artifact: DeployRequest,
    ) -> Result<(), ExecutionError>;
    /// Confirmation that the artifact was successfully deployed by the validator.
    ///
    /// Artifact will be registered in dispatcher if all of validators will send this confirmation.
    fn confirm_artifact_deploy(
        &self,
        context: TransactionContext,
        artifact: DeployConfirmation,
    ) -> Result<(), ExecutionError>;
    /// Requests start service.
    ///
    /// Service will be started if all of validators will send this confirmation.
    fn start_service(
        &self,
        context: TransactionContext,
        service: StartService,
    ) -> Result<(), ExecutionError>;
}

impl ValidateInput for DeployRequest {
    type Error = ExecutionError;

    fn validate(&self) -> Result<(), Self::Error> {
        self.artifact
            .validate()
            .map_err(|e| (Error::InvalidArtifactId, e).into())
    }
}

impl ValidateInput for DeployConfirmation {
    type Error = ExecutionError;

    fn validate(&self) -> Result<(), Self::Error> {
        self.artifact
            .validate()
            .map_err(|e| (Error::InvalidArtifactId, e).into())
    }
}

impl ValidateInput for StartService {
    type Error = ExecutionError;

    fn validate(&self) -> Result<(), Self::Error> {
        self.artifact
            .validate()
            .map_err(|e| (Error::InvalidArtifactId, e))?;
        InstanceSpec::is_valid_name(&self.name)
            .map_err(|e| (Error::InvalidInstanceName, e))
            .map_err(Self::Error::from)
    }
}

impl Transactions for Supervisor {
    fn request_artifact_deploy(
        &self,
        context: TransactionContext,
        deploy: DeployRequest,
    ) -> Result<(), ExecutionError> {
        deploy.validate()?;
        let blockchain_schema = blockchain::Schema::new(context.fork());
        // Verifies that we doesn't reach deadline height.
        if deploy.deadline_height < blockchain_schema.height() {
            return Err(Error::DeadlineExceeded)?;
        }
        let schema = Schema::new(context.service_name(), context.fork());

        // Verifies that the deployment request is not yet registered.
        if schema.pending_deployments().contains(&deploy.artifact) {
            return Err(Error::DeployRequestAlreadyRegistered)?;
        }

        // Verifies that transaction author is validator.
        let mut deploy_requests = schema.deploy_requests();
        let author = context.author();
        deploy_requests
            .validator_id(author)
            .ok_or(Error::UnknownAuthor)?;

        // Verifies that the artifact is not deployed yet.
        if dispatcher::Schema::new(context.fork())
            .artifacts()
            .contains(&deploy.artifact.name)
        {
            return Err(Error::AlreadyDeployed)?;
        }

        let confirmations = deploy_requests.confirm(&deploy, author);
        if confirmations == deploy_requests.validators_len() {
            trace!("Deploy artifact request accepted {:?}", deploy.artifact);

            let artifact = deploy.artifact.clone();
            schema.pending_deployments().put(&artifact, deploy);
        }
        Ok(())
    }

    fn confirm_artifact_deploy(
        &self,
        mut context: TransactionContext,
        confirmation: DeployConfirmation,
    ) -> Result<(), ExecutionError> {
        confirmation.validate()?;
        let blockchain_schema = blockchain::Schema::new(context.fork());

        // Verifies that we doesn't reach deadline height.
        if confirmation.deadline_height < blockchain_schema.height() {
            return Err(Error::DeadlineExceeded)?;
        }
        let schema = Schema::new(context.service_name(), context.fork());

        // Verifies that transaction author is validator.
        let mut deploy_confirmations = schema.deploy_confirmations();
        let author = context.author();
        deploy_confirmations
            .validator_id(author)
            .ok_or(Error::UnknownAuthor)?;

        // Verifies that this deployment is registered.
        if !schema
            .pending_deployments()
            .contains(&confirmation.artifact)
        {
            return Err(Error::DeployRequestNotRegistered)?;
        }

        let confirmations = deploy_confirmations.confirm(&confirmation, author);
        if confirmations == deploy_confirmations.validators_len() {
            trace!(
                "Registering deployed artifact in dispatcher {:?}",
                confirmation.artifact
            );

            // Removes artifact from pending deployments.
            schema.pending_deployments().remove(&confirmation.artifact);
            // We have enough confirmations to register the deployed artifact in the dispatcher,
            // if this action fails this transaction will be canceled.
            context.dispatch_action(Action::RegisterArtifact {
                artifact: confirmation.artifact,
                spec: confirmation.spec,
            });
        }

        Ok(())
    }

    fn start_service(
        &self,
        mut context: TransactionContext,
        service: StartService,
    ) -> Result<(), ExecutionError> {
        service.validate()?;
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
            trace!(
                "Request start service with name {:?} from artifact {:?}",
                service.name,
                service.artifact
            );
            // We have enough confirmations to start a new service instance,
            // if this action fails this transaction will be canceled.
            context.dispatch_action(Action::StartService {
                artifact: service.artifact,
                instance_name: service.name,
                config: service.config,
            })
        }

        Ok(())
    }
}

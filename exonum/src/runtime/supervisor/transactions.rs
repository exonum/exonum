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
    blockchain::{self, ExecutionError, ExecutionResult},
    runtime::{
        dispatcher::{self, Action},
        rust::TransactionContext,
    },
};

use super::{DeployArtifact, Schema, StartService, Supervisor};

/// Common errors emitted by transactions during execution.
#[derive(Debug, Fail)]
#[repr(u8)]
pub enum Error {
    /// Artifact has been already deployed.
    #[fail(display = "Artifact has been already deployed")]
    AlreadyDeployed = 0,
    /// Transaction author is not a validator.
    #[fail(display = "Transaction author is not a validator")]
    UnknownAuthor = 1,
    /// Reached deadline for deploying artifact.
    #[fail(display = "Reached deadline for deploying artifact")]
    DeployDeadline = 2,
}

impl From<Error> for ExecutionError {
    fn from(value: Error) -> ExecutionError {
        let description = value.to_string();
        ExecutionError::with_description(value as u8, description)
    }
}

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
            .deployed_artifacts()
            .contains(&artifact.id.name)
        {
            return Err(Error::AlreadyDeployed)?;
        }

        // Verifies that we doesn't reach deadline height.
        if artifact.deadline_height > blockchain_schema.height() {
            return Err(Error::DeployDeadline)?;
        }

        let confirmations = Schema::new(context.service_name(), context.fork())
            .confirm_pending_artifact(&artifact.id, context.author());
        if confirmations == validator_keys.len() {
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

    fn start_service(&self, context: TransactionContext, arg: StartService) -> ExecutionResult {
        unimplemented!()
    }
}

impl_service_dispatcher!(Supervisor, Transactions);

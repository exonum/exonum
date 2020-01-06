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

use exonum::{
    crypto::PublicKey,
    helpers::{Height, ValidateInput},
    runtime::{
        rust::CallContext, CommonError, ExecutionError, ExecutionFail, InstanceSpec, InstanceStatus,
    },
};
use exonum_derive::*;
use exonum_merkledb::ObjectHash;

use std::collections::HashSet;

use super::{
    configure::ConfigureMut, ConfigChange, ConfigProposalWithHash, ConfigPropose, ConfigVote,
    DeployRequest, DeployResult, DeployState, Error, SchemaImpl, StartService, StopService,
    Supervisor,
};

/// Supervisor service transactions.
#[exonum_interface]
pub trait SupervisorInterface<Ctx> {
    /// Output generated by the stub.
    type Output;

    /// Requests artifact deploy.
    ///
    /// This request should be initiated by the validator (and depending on the `Supervisor`
    /// mode several other actions can be required, e.g. sending the same request by majority
    /// of other validators as well).
    /// After that, the supervisor will try to deploy the artifact, and if this procedure
    /// will be successful it will send `confirm_artifact_deploy` transaction.
    fn request_artifact_deploy(&self, context: Ctx, artifact: DeployRequest) -> Self::Output;

    /// Confirmation that the artifact was successfully deployed by the validator.
    ///
    /// The artifact is registered in the dispatcher if all validators send this confirmation.
    /// This confirmation is sent automatically by the node if the deploy succeeds.
    fn confirm_artifact_deploy(&self, context: Ctx, artifact: DeployResult) -> Self::Output;

    /// Propose config change
    ///
    /// This request should be sent by one of validators as the proposition to change
    /// current configuration to new one. All another validators are able to vote for this
    /// configuration by sending `confirm_config_change` transaction.
    /// The configuration application rules depend on the `Supervisor` mode, e.g. confirmations
    /// are not required for the `Simple` mode, and for `Decentralized` mode (2/3+1) confirmations
    /// are required.
    ///
    /// **Note:** only one proposal at time is possible.
    fn propose_config_change(&self, context: Ctx, propose: ConfigPropose) -> Self::Output;

    /// Confirm config change
    ///
    /// This confirm should be sent by validators to vote for proposed configuration.
    /// Vote of the author of the `propose_config_change` transaction is taken into
    /// account automatically.
    /// The configuration application rules depend on the `Supervisor` mode.
    fn confirm_config_change(&self, context: Ctx, vote: ConfigVote) -> Self::Output;
}

impl StartService {
    fn validate(&self, context: &CallContext<'_>) -> Result<(), ExecutionError> {
        self.artifact
            .validate()
            .map_err(|e| Error::InvalidArtifactId.with_description(e))?;
        InstanceSpec::is_valid_name(&self.name)
            .map_err(|e| Error::InvalidInstanceName.with_description(e))?;

        let dispatcher_data = context.data().for_dispatcher();

        // Check that artifact is deployed.
        if dispatcher_data.get_artifact(&self.artifact).is_none() {
            log::trace!(
                "Discarded start of service {} from the unknown artifact {}.",
                &self.name,
                &self.artifact.name,
            );

            let err = Error::UnknownArtifact.with_description(format!(
                "Discarded start of service {} from the unknown artifact {}.",
                &self.name, &self.artifact.name,
            ));
            return Err(err);
        }

        // Check that there is no instance with the same name.
        if dispatcher_data.get_instance(self.name.as_str()).is_some() {
            return Err(Error::InstanceExists.with_description(format!(
                "Discarded an attempt to start of the already started instance {}.",
                &self.name
            )));
        }

        Ok(())
    }
}

impl StopService {
    fn validate(&self, context: &CallContext<'_>) -> Result<(), ExecutionError> {
        let instance = context
            .data()
            .for_dispatcher()
            .get_instance(self.instance_id)
            .ok_or_else(|| {
                Error::MalformedConfigPropose
                    .with_description("Instance with the specified ID is absent.")
            })?;

        match instance.status {
            Some(InstanceStatus::Active) => Ok(()),
            _ => Err(Error::MalformedConfigPropose.with_description(format!(
                "Discarded an attempt to stop the already stopped service instance: {}",
                instance.spec.name
            ))),
        }
    }
}

impl SupervisorInterface<CallContext<'_>> for Supervisor {
    type Output = Result<(), ExecutionError>;

    fn propose_config_change(
        &self,
        mut context: CallContext<'_>,
        mut propose: ConfigPropose,
    ) -> Self::Output {
        let author = context
            .caller()
            .author()
            .ok_or(CommonError::UnauthorizedCaller)?;

        // Verifies that transaction author is validator.
        context
            .data()
            .for_core()
            .validator_id(author)
            .ok_or(Error::UnknownAuthor)?;

        let current_height = context.data().for_core().height();

        // If `actual_from` field is not set, set it to the next height.
        if propose.actual_from == Height(0) {
            propose.actual_from = current_height.next();
        }
        // Otherwise verify that the `actual_from` height is in the future.
        else if current_height >= propose.actual_from {
            return Err(Error::ActualFromIsPast.into());
        }

        let mut schema = SchemaImpl::new(context.service_data());

        // Verifies that there are no pending config changes.
        if let Some(proposal) = schema.public.pending_proposal.get() {
            // We have a proposal, check that it's actual.
            if current_height < proposal.config_propose.actual_from {
                return Err(Error::ConfigProposeExists.into());
            } else {
                // Proposal is outdated but was not removed (e.g. because of the panic
                // during config applying), clean it.
                schema.public.pending_proposal.remove();
            }
        }
        drop(schema);

        // Verify changes in the proposal.
        self.verify_config_changeset(&mut context, &propose.changes)?;
        let mut schema = SchemaImpl::new(context.service_data());

        // After all the checks verify that configuration number is expected one.
        if propose.configuration_number != schema.get_configuration_number() {
            return Err(Error::IncorrectConfigurationNumber.into());
        }
        schema.increase_configuration_number();

        let propose_hash = propose.object_hash();
        schema.config_confirms.confirm(&propose_hash, author);

        let config_entry = ConfigProposalWithHash {
            config_propose: propose,
            propose_hash,
        };
        schema.public.pending_proposal.set(config_entry);

        Ok(())
    }

    fn confirm_config_change(&self, context: CallContext<'_>, vote: ConfigVote) -> Self::Output {
        let (_, author) = context
            .caller()
            .as_transaction()
            .ok_or(CommonError::UnauthorizedCaller)?;

        // Verify that transaction author is a validator.
        let core_schema = context.data().for_core();
        core_schema
            .validator_id(author)
            .ok_or(Error::UnknownAuthor)?;

        let mut schema = SchemaImpl::new(context.service_data());
        let entry = schema
            .public
            .pending_proposal
            .get()
            .ok_or(Error::ConfigProposeNotRegistered)?;

        // Verifies that this config proposal is registered.
        if entry.propose_hash != vote.propose_hash {
            return Err(Error::ConfigProposeNotRegistered.into());
        }

        let config_propose = entry.config_propose;
        // Verifies that we didn't reach the deadline height.
        if config_propose.actual_from <= core_schema.height() {
            return Err(Error::DeadlineExceeded.into());
        }
        if schema
            .config_confirms
            .confirmed_by(&entry.propose_hash, &author)
        {
            return Err(Error::AttemptToVoteTwice.into());
        }

        schema.config_confirms.confirm(&vote.propose_hash, author);
        log::trace!(
            "Propose config {:?} has been confirmed by {:?}",
            vote.propose_hash,
            author
        );

        Ok(())
    }

    fn request_artifact_deploy(
        &self,
        context: CallContext<'_>,
        deploy: DeployRequest,
    ) -> Self::Output {
        deploy
            .artifact
            .validate()
            .map_err(|e| Error::InvalidArtifactId.with_description(e))?;

        let core_schema = context.data().for_core();
        let validator_count = core_schema.consensus_config().validator_keys.len();
        // Verifies that we doesn't reach deadline height.
        if deploy.deadline_height < core_schema.height() {
            return Err(Error::ActualFromIsPast.into());
        }
        let mut schema = SchemaImpl::new(context.service_data());

        // Verifies that transaction author is validator.
        let author = context.caller().author().ok_or(Error::UnknownAuthor)?;
        core_schema
            .validator_id(author)
            .ok_or(Error::UnknownAuthor)?;

        // Verifies that the artifact is not deployed yet.
        if context
            .data()
            .for_dispatcher()
            .get_artifact(&deploy.artifact)
            .is_some()
        {
            return Err(Error::AlreadyDeployed.into());
        }

        // If deployment is already registered, check whether request is initiated
        if schema.pending_deployments.contains(&deploy.artifact) {
            let new_confirmation = !schema.deploy_requests.confirmed_by(&deploy, &author);
            if new_confirmation {
                // It's OK, just an additional confirmation.
                schema.deploy_requests.confirm(&deploy, author);
                return Ok(());
            } else {
                // Author already confirmed deployment of this artifact,
                // so it's a duplicate.
                return Err(Error::DeployRequestAlreadyRegistered.into());
            }
        }

        schema.deploy_requests.confirm(&deploy, author);
        let supervisor_mode = schema.supervisor_config().mode;
        if supervisor_mode.deploy_approved(&deploy, &schema.deploy_requests, validator_count) {
            schema.deploy_states.put(&deploy, DeployState::Pending);
            log::trace!("Deploy artifact request accepted {:?}", deploy.artifact);
            let artifact = deploy.artifact.clone();
            schema.pending_deployments.put(&artifact, deploy);
        }
        Ok(())
    }

    fn confirm_artifact_deploy(
        &self,
        context: CallContext<'_>,
        deploy_result: DeployResult,
    ) -> Self::Output {
        deploy_result
            .request
            .artifact
            .validate()
            .map_err(|e| Error::InvalidArtifactId.with_description(e))?;

        let core_schema = context.data().for_core();

        // Verify that transaction author is validator.
        let author = context
            .caller()
            .author()
            .ok_or(CommonError::UnauthorizedCaller)?;
        core_schema
            .validator_id(author)
            .ok_or(Error::UnknownAuthor)?;
        let current_height = core_schema.height();

        let schema = SchemaImpl::new(context.service_data());

        // Check if deployment already failed.
        if schema
            .deploy_states
            .get(&deploy_result.request)
            .map(|state| state.is_failed())
            .unwrap_or_default()
        {
            // This deployment is already resulted in failure, no further
            // processing needed.
            return Ok(());
        }

        // Verify that this deployment is registered.
        let deploy_request = schema
            .pending_deployments
            .get(&deploy_result.request.artifact)
            .ok_or(Error::DeployRequestNotRegistered)?;

        // Check that pending deployment is the same as in confirmation.
        if deploy_request != deploy_result.request {
            let error = ExecutionError::from(Error::DeployRequestNotRegistered);
            return Err(error);
        }

        // Verify that we didn't reach deadline height.
        if deploy_request.deadline_height < current_height {
            return Err(Error::DeadlineExceeded.into());
        }

        drop(schema);
        match deploy_result.result.0 {
            Ok(()) => self.confirm_deploy(context, deploy_request, author)?,
            Err(error) => self.fail_deploy(context, deploy_request, error),
        }
        Ok(())
    }
}

impl Supervisor {
    /// Verifies that each change introduced within config proposal is valid.
    fn verify_config_changeset(
        &self,
        context: &mut CallContext<'_>,
        changes: &[ConfigChange],
    ) -> Result<(), ExecutionError> {
        // To prevent multiple consensus change proposition in one request
        let mut consensus_propose_added = false;
        // To prevent multiple service change proposition in one request
        let mut modified_instances = HashSet::new();
        // To prevent multiple services start in one request.
        let mut services_to_start = HashSet::new();

        // Perform config verification.
        for change in changes {
            match change {
                ConfigChange::Consensus(config) => {
                    if consensus_propose_added {
                        return Err(Error::MalformedConfigPropose.with_description(
                            "Discarded multiple consensus change proposals in one request.",
                        ));
                    }
                    consensus_propose_added = true;
                    config
                        .validate()
                        .map_err(|e| Error::MalformedConfigPropose.with_description(e))?;
                }

                ConfigChange::Service(config) => {
                    if !modified_instances.insert(config.instance_id) {
                        return Err(Error::MalformedConfigPropose.with_description(
                            "Discarded multiple service change proposals in one request.",
                        ));
                    }

                    context.verify_config(config.instance_id, config.params.clone())?;
                }

                ConfigChange::StartService(start_service) => {
                    if !services_to_start.insert(start_service.name.clone()) {
                        return Err(Error::MalformedConfigPropose.with_description(
                            "Discarded multiple instances with the same name in one request.",
                        ));
                    }
                    start_service.validate(&context)?;
                }

                ConfigChange::StopService(stop_service) => {
                    if !modified_instances.insert(stop_service.instance_id) {
                        return Err(Error::MalformedConfigPropose.with_description(
                            "Discarded multiple instances with the same name in one request.",
                        ));
                    }
                    stop_service.validate(&context)?;
                }
            }
        }
        Ok(())
    }

    /// Confirms a deploy by the given author's public key and checks
    /// if all the confirmations are collected. If so, starts the artifact registration.
    fn confirm_deploy(
        &self,
        context: CallContext<'_>,
        deploy_request: DeployRequest,
        author: PublicKey,
    ) -> Result<(), ExecutionError> {
        let core_schema = context.data().for_core();

        let mut schema = SchemaImpl::new(context.service_data());

        let confirmations = schema.deploy_confirmations.confirm(&deploy_request, author);
        let validator_count = core_schema.consensus_config().validator_keys.len();
        if confirmations == validator_count {
            log::trace!(
                "Registering deployed artifact in dispatcher {:?}",
                deploy_request.artifact
            );

            // Remove artifact from pending deployments.
            schema.pending_deployments.remove(&deploy_request.artifact);
            schema
                .deploy_states
                .put(&deploy_request, DeployState::Succeed);
            // We have enough confirmations to register the deployed artifact in the dispatcher;
            // if this action fails, this transaction will be canceled.
            context.start_artifact_registration(deploy_request.artifact, deploy_request.spec);
        }
        Ok(())
    }

    /// Marks deployment as failed, discarding the further deployment steps.
    fn fail_deploy(
        &self,
        context: CallContext<'_>,
        deploy_request: DeployRequest,
        error: ExecutionError,
    ) {
        let height = context.data().for_core().height();
        let mut schema = SchemaImpl::new(context.service_data());

        // Mark deploy as failed.
        schema
            .deploy_states
            .put(&deploy_request, DeployState::Failed { height, error });

        // Remove artifact from pending deployments: since we require
        // a confirmation from every node, failure for one node means failure
        // for the whole network.
        schema.pending_deployments.remove(&deploy_request.artifact);
    }
}

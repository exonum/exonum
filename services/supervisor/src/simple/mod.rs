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

//! A simple supervisor service that does actions without any confirmations.

use exonum::{
    blockchain::{self, InstanceCollection},
    crypto::Hash,
    helpers::{validator::validator_id as find_validator_id, ValidateInput},
    merkledb::Snapshot,
    runtime::{
        api::ServiceApiBuilder,
        rust::{CallContext, Service},
        DispatcherError, ExecutionError, InstanceDescriptor, InstanceSpec, SUPERVISOR_INSTANCE_ID,
    },
};
use exonum_derive::{exonum_service, IntoExecutionError, ServiceFactory};

use crate::{
    apply_consensus_config, apply_service_config, ConfigChange, ConfigPropose, ConfigureCall,
};

mod api;
mod schema;
pub use self::schema::Schema;

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
fn update_configs(context: &mut CallContext, changes: Vec<ConfigChange>) {
    // An error while configuring one of the service instances should not affect others.
    changes.into_iter().for_each(|change| match change {
        ConfigChange::Consensus(config) => {
            log::trace!("Updating consensus configuration {:?}", config);
            let configure_result = apply_consensus_config(context, config);

            if let Err(e) = configure_result {
                // Consensus configuration is not allowed to fail, since it was validated and approved.
                // This is an unrecoverable error.
                panic!("Consensus configuration failed with error {:?}", e)
            }
        }

        ConfigChange::Service(config) => {
            log::trace!(
                "Updating service instance configuration, instance ID is {}",
                config.instance_id
            );
            let configure_result = apply_service_config(context, config);

            if let Err(e) = configure_result {
                log::error!(
                    "An error occurred while applying service configuration. {}",
                    e
                );
            }
        }

        ConfigChange::DeployRequest(deploy_request) => {
            log::trace!("Processing deploy request {:?}", &deploy_request);
            // Activation height is ignored for simple supervisor.
            let start_deploy_result =
                context.start_artifact_registration(deploy_request.artifact, deploy_request.spec);
            if let Err(e) = start_deploy_result {
                // Deployment failures are not allowed within simple supervisor.
                panic!("Deployment failed with error {:?}", e);
            }
        }

        ConfigChange::StartService(start_request) => {
            log::trace!("Processing start service request {:?}", &start_request);
            let start_instance_result = context.start_adding_service(
                start_request.artifact,
                start_request.name,
                start_request.config,
            );
            if let Err(e) = start_instance_result {
                // Service start failures are not allowed within simple supervisor.
                panic!("Service start failed with error {:?}", e);
            }
        }
    })
}

/// Simple supervisor for testing purposes. **This supervisor is not fit for real-world use cases.**
#[derive(Debug, ServiceFactory)]
#[exonum(
    proto_sources = "crate::proto",
    artifact_name = "simple-supervisor",
    implements("SimpleSupervisorInterface")
)]
pub struct SimpleSupervisor;

#[derive(Debug, Copy, Clone, IntoExecutionError)]
pub enum Error {
    /// Malformed configuration change proposal.
    MalformedConfigPropose = 0,
    /// Active configuration change proposal already exists.
    ConfigProposeExists = 1,
    /// Invalid consensus configuration.
    ConsensusConfigInvalid = 3,
    /// Actual height for config proposal is in the past.
    ActualFromIsPast = 4,
    /// Transaction author is not a validator.
    UnknownAuthor = 5,
    /// Artifact has been already deployed.
    AlreadyDeployed = 6,
    /// Instance with the given name already exists.
    InstanceExists = 7,
    /// Artifact identifier has incorrect format.
    InvalidArtifactId = 8,
    /// Instance name is incorrect.
    InvalidInstanceName = 9,
    /// Unknown artifact in the `StartService` request.
    UnknownArtifact = 10,
}

#[exonum_service]
pub trait SimpleSupervisorInterface {
    fn change_config(&self, context: CallContext, arg: ConfigPropose)
        -> Result<(), ExecutionError>;
}

impl SimpleSupervisorInterface for SimpleSupervisor {
    fn change_config(
        &self,
        mut context: CallContext,
        propose: ConfigPropose,
    ) -> Result<(), ExecutionError> {
        // Verify that transaction author is validator.
        let author = context
            .caller()
            .author()
            .ok_or(DispatcherError::UnauthorizedCaller)?;
        find_validator_id(context.fork().as_ref(), author).ok_or(Error::UnknownAuthor)?;

        // Check that the `actual_from` height is in the future.
        if blockchain::Schema::new(context.fork()).height() >= propose.actual_from {
            log::warn!("Received config proposal with incorrect height");
            return Err(Error::ActualFromIsPast).map_err(From::from);
        }

        let schema = Schema::new(context.fork());
        // Check that there are no pending config changes.
        if schema.config_propose_entry().exists() {
            log::warn!("Received config proposal while pending proposal already exists");
            return Err(Error::ConfigProposeExists).map_err(From::from);
        }

        log::info!("Received a new config proposal: {:?}", &propose);

        // Perform config verification.
        for change in &propose.changes {
            match change {
                ConfigChange::Consensus(config) => {
                    config.validate().map_err(|e| {
                        log::warn!("Consensus config entry validation failed: {:?}", &e);
                        (Error::ConsensusConfigInvalid, e)
                    })?;
                }

                ConfigChange::Service(config) => {
                    context
                        .interface::<ConfigureCall>(config.instance_id)?
                        .verify_config(config.params.clone())
                        .map_err(|e| {
                            log::warn!("Service config entry validation failed: {:?}", &e);
                            (Error::MalformedConfigPropose, e)
                        })?;
                }
                ConfigChange::DeployRequest(request) => {
                    request.artifact.validate().map_err(|e| {
                        log::warn!("Invalid artifact name {:?}", &request.artifact);
                        ExecutionError::from((Error::InvalidArtifactId, e))
                    })?;

                    if context
                        .dispatcher_info()
                        .get_artifact(&request.artifact.name)
                        .is_some()
                    {
                        log::warn!("Artifact {:?} is already deployed", &request.artifact);
                        return Err(Error::AlreadyDeployed.into());
                    }
                }
                ConfigChange::StartService(request) => {
                    request.artifact.validate().map_err(|e| {
                        log::warn!(
                            "Invalid artifact name within StartService request: {:?}",
                            &request.artifact
                        );
                        ExecutionError::from((Error::InvalidArtifactId, e))
                    })?;

                    InstanceSpec::is_valid_name(&request.name).map_err(|e| {
                        log::warn!("Invalid instance name: {:?}", &request.name);
                        ExecutionError::from((Error::InvalidInstanceName, e))
                    })?;

                    if context
                        .dispatcher_info()
                        .get_artifact(&request.artifact.name)
                        .is_none()
                    {
                        log::warn!(
                            "Received StartService request for unknown artifact: {:?}",
                            &request.artifact
                        );
                        return Err(Error::UnknownArtifact.into());
                    }

                    // Verifies that the instance name does not exist.
                    if context
                        .dispatcher_info()
                        .get_instance(request.name.as_str())
                        .is_some()
                    {
                        log::warn!(
                            "Instance with name '{:?}' is already running",
                            &request.name
                        );
                        return Err(Error::InstanceExists.into());
                    }
                }
            }
        }

        log::info!(
            "Proposal confirmed and set as pending. It will be applied at the height {}",
            propose.actual_from
        );

        // Add verified config proposal to the pending config changes.
        let schema = Schema::new(context.fork());
        schema.config_propose_entry().set(propose);
        Ok(())
    }
}

impl Service for SimpleSupervisor {
    fn state_hash(&self, _instance: InstanceDescriptor, snapshot: &dyn Snapshot) -> Vec<Hash> {
        Schema::new(snapshot).state_hash()
    }

    fn before_commit(&self, mut context: CallContext) {
        let schema = Schema::new(context.fork());
        let proposal = if let Some(proposal) =
            schema.config_propose_entry().get().filter(|proposal| {
                proposal.actual_from == blockchain::Schema::new(context.fork()).height().next()
            }) {
            proposal
        } else {
            return;
        };

        // Perform the application of configs.
        update_configs(&mut context, proposal.changes);

        // Remove config from proposals.
        let schema = Schema::new(context.fork());
        schema.config_propose_entry().remove();
    }

    fn wire_api(&self, builder: &mut ServiceApiBuilder) {
        api::wire(builder)
    }
}

impl SimpleSupervisor {
    pub const BUILTIN_NAME: &'static str = "simple-supervisor";
}

impl From<SimpleSupervisor> for InstanceCollection {
    fn from(inner: SimpleSupervisor) -> Self {
        Self::new(inner).with_instance(
            SUPERVISOR_INSTANCE_ID,
            SimpleSupervisor::BUILTIN_NAME,
            Vec::default(),
        )
    }
}

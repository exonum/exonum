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
    helpers::ValidateInput,
    merkledb::Snapshot,
    runtime::{
        rust::{CallContext, Service},
        Caller, DispatcherError, ExecutionError, InstanceDescriptor, SUPERVISOR_INSTANCE_ID,
    },
};
use exonum_derive::{exonum_service, IntoExecutionError, ServiceFactory};

use crate::{update_configs, ConfigChange, ConfigPropose, ConfigureCall};

mod schema;
pub use self::schema::Schema;

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
}

#[exonum_service]
pub trait SimpleSupervisorInterface {
    fn change_config(&self, context: CallContext, arg: ConfigPropose)
        -> Result<(), ExecutionError>;
}

impl SimpleSupervisorInterface for SimpleSupervisor {
    // TODO: check auth by one of validators [ECR-3742]
    fn change_config(
        &self,
        mut context: CallContext,
        arg: ConfigPropose,
    ) -> Result<(), ExecutionError> {
        context
            .verify_caller(Caller::as_transaction)
            .ok_or(DispatcherError::UnauthorizedCaller)?;

        // Check that the `actual_from` height is in the future.
        if blockchain::Schema::new(context.fork()).height() >= arg.actual_from {
            return Err(Error::ActualFromIsPast).map_err(From::from);
        }

        let schema = Schema::new(context.fork());
        // Check that there are no pending config changes.
        if schema.config_propose_entry().exists() {
            return Err(Error::ConfigProposeExists).map_err(From::from);
        }

        // Perform config verification.
        for change in &arg.changes {
            match change {
                ConfigChange::Consensus(config) => {
                    config
                        .validate()
                        .map_err(|e| (Error::ConsensusConfigInvalid, e))?;
                }

                ConfigChange::Service(config) => {
                    context
                        .interface::<ConfigureCall>(config.instance_id)?
                        .verify_config(config.params.clone())
                        .map_err(|e| (Error::MalformedConfigPropose, e))?;
                }
            }
        }

        // Add verified config proposal to the pending config changes.
        let schema = Schema::new(context.fork());
        schema.config_propose_entry().set(arg);
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

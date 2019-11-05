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
    blockchain::InstanceCollection,
    crypto::Hash,
    helpers::ValidateInput,
    merkledb::Snapshot,
    runtime::{
        api::ServiceApiBuilder,
        rust::{CallContext, Service},
        BlockchainData, DispatcherError, ExecutionError, SUPERVISOR_INSTANCE_ID,
    },
};
use exonum_derive::{exonum_service, IntoExecutionError, ServiceFactory};

use crate::{update_configs, ConfigChange, ConfigPropose, ConfigureCall};

mod api;
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
    /// Transaction author is not a validator.
    UnknownAuthor = 5,
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
        arg: ConfigPropose,
    ) -> Result<(), ExecutionError> {
        // Verify that transaction author is validator.
        let author = context
            .caller()
            .author()
            .ok_or(DispatcherError::UnauthorizedCaller)?;
        context
            .data()
            .for_core()
            .validator_id(author)
            .ok_or(Error::UnknownAuthor)?;

        // Check that the `actual_from` height is in the future.
        if context.data().for_core().height() >= arg.actual_from {
            Err(Error::ActualFromIsPast)?;
        }

        // Check that there are no pending config changes.
        if Schema::new(context.service_data()).config_propose.exists() {
            Err(Error::ConfigProposeExists)?;
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
        let mut schema = Schema::new(context.service_data());
        schema.config_propose.set(arg);
        Ok(())
    }
}

impl Service for SimpleSupervisor {
    fn initialize(&self, context: CallContext<'_>, _params: Vec<u8>) -> Result<(), ExecutionError> {
        Schema::ensure(context.service_data());
        Ok(())
    }

    fn state_hash(&self, data: BlockchainData<&'_ dyn Snapshot>) -> Vec<Hash> {
        Schema::new(data.for_executing_service()).state_hash()
    }

    fn before_commit(&self, mut context: CallContext<'_>) {
        let proposal = Schema::new(context.service_data())
            .config_propose
            .get()
            .filter(|proposal| {
                let height = context.data().for_core().height();
                proposal.actual_from == height.next()
            });
        let proposal = if let Some(proposal) = proposal {
            proposal
        } else {
            return;
        };

        // Perform the application of configs.
        update_configs(&mut context, proposal.changes);
        // Remove config from proposals.
        let mut schema = Schema::new(context.service_data());
        schema.config_propose.remove();
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

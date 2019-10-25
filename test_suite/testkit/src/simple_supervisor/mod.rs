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

pub use proto::ConfigPropose;
pub use schema::Schema;

use exonum::{
    blockchain::{self, InstanceCollection},
    crypto::Hash,
    helpers::ValidateInput,
    merkledb::Snapshot,
    runtime::{
        rust::{interfaces::ConfigureCall, BeforeCommitContext, Service, TransactionContext},
        Caller, ConfigChange, DispatcherError, ExecutionError, InstanceDescriptor, InstanceId,
    },
};
use exonum_derive::{exonum_service, IntoExecutionError, ServiceFactory};

mod proto;
mod schema;
#[cfg(test)]
pub mod tests;

#[derive(Debug, ServiceFactory)]
#[exonum(
    proto_sources = "proto::schema",
    artifact_name = "exonum-testkit-supervisor",
    implements("Transactions")
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
pub trait Transactions {
    fn change_config(
        &self,
        context: TransactionContext,
        arg: ConfigPropose,
    ) -> Result<(), ExecutionError>;
}

impl Transactions for SimpleSupervisor {
    fn change_config(
        &self,
        context: TransactionContext,
        arg: ConfigPropose,
    ) -> Result<(), ExecutionError> {
        let (_, fork) = context
            .verify_caller(Caller::as_transaction)
            .ok_or(DispatcherError::UnauthorizedCaller)?;

        // Check that the `actual_from` height is in the future.
        if blockchain::Schema::new(fork).height() >= arg.actual_from {
            return Err(Error::ActualFromIsPast).map_err(From::from);
        }

        let schema = Schema::new(fork);
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
                        .interface::<ConfigureCall>(config.instance_id)
                        .verify_config(config.params.clone())
                        .map_err(|e| (Error::MalformedConfigPropose, e))?;
                }
            }
        }
        // Add verified config proposal to the pending config changes.
        schema.config_propose_entry().set(arg);
        Ok(())
    }
}

impl Service for SimpleSupervisor {
    fn state_hash(&self, _instance: InstanceDescriptor, snapshot: &dyn Snapshot) -> Vec<Hash> {
        Schema::new(snapshot).state_hash()
    }

    fn before_commit(&self, context: BeforeCommitContext) {
        let schema = Schema::new(context.fork);
        let proposal = if let Some(proposal) =
            schema.config_propose_entry().get().filter(|proposal| {
                proposal.actual_from == blockchain::Schema::new(context.fork).height().next()
            }) {
            proposal
        } else {
            return;
        };
        // Perform the application of configs.
        context.update_config(proposal.changes);
        // Remove config from proposals.
        schema.config_propose_entry().remove();
    }
}

impl SimpleSupervisor {
    pub const BUILTIN_ID: InstanceId = 0;
    pub const BUILTIN_NAME: &'static str = "supervisor-sample";
}

impl From<SimpleSupervisor> for InstanceCollection {
    fn from(inner: SimpleSupervisor) -> Self {
        Self::new(inner).with_instance(
            SimpleSupervisor::BUILTIN_ID,
            SimpleSupervisor::BUILTIN_NAME,
            Vec::default(),
        )
    }
}

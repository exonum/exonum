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

// use semver::Version;

use crate::{
    blockchain::Schema as CoreSchema,
    messages::BinaryForm,
    runtime::{
        error::{ExecutionError, WRONG_ARG_ERROR},
        rust::{service::Service, TransactionContext},
    },
    node::State,
    proto::schema::configuration::ConfigurationServiceInit,
};
use protobuf::{well_known_types::Any};

mod transactions;
mod config;
mod errors;
mod schema;

use transactions::{VotingContext, enough_votes_to_commit};
use schema::VotingDecision;
use errors::Error as ServiceError;

/// Service identifier for the configuration service.
pub const SERVICE_ID: u16 = 1;
/// Configuration service name.
pub const SERVICE_NAME: &str = "configuration";

service_interface! {
    trait ConfigurationService {
        fn propose(&self, ctx: TransactionContext, tx: transactions::Propose) -> Result<(), ExecutionError>;
        fn vote(&self, ctx: TransactionContext, arg: transactions::Vote) -> Result<(), ExecutionError>;
        fn vote_against(&self, ctx: TransactionContext, arg: transactions::VoteAgainst) -> Result<(), ExecutionError>;
    }
}

#[derive(Debug, Default)]
pub struct ConfigurationServiceImpl {
    pub majority_count: Option<u32>,
}


impl ConfigurationService for ConfigurationServiceImpl {
    fn propose(&self, mut ctx: TransactionContext, tx: transactions::Propose) -> Result<(), ExecutionError>  {
        let author = ctx.author();
        let fork = ctx.fork();
        let (cfg, cfg_hash) = tx.precheck(fork.as_ref(), author).map_err(|err| {
            error!("Discarding propose {:?}: {}", self, err);
            err
        })?;

        tx.save(fork, &cfg, cfg_hash);
        trace!("Put propose {:?} to config_proposes table", self);
        Ok(())
    }


    fn vote(&self, mut ctx: TransactionContext, tx: transactions::Vote) -> Result<(), ExecutionError> {
        let author = ctx.author();
        let tx_hash = ctx.tx_hash();
        let fork = ctx.fork();
        let decision = VotingDecision::Yea(tx_hash);

        let vote = VotingContext::new(decision, author, tx.cfg_hash);
        let parsed_config = vote.precheck(fork.as_ref()).map_err(|err| {
            error!("Discarding vote {:?}: {}", tx, err);
            err
        })?;

        vote.save(fork);
        trace!(
            "Put Vote:{:?} to corresponding cfg votes_by_config_hash table",
            tx
        );

        if enough_votes_to_commit(fork.as_ref(), &tx.cfg_hash) {
            CoreSchema::new(fork).commit_configuration(parsed_config);
        }
        Ok(())
    }

    fn vote_against(&self, mut ctx: TransactionContext, tx: transactions::VoteAgainst) -> Result<(), ExecutionError> {
        let author = ctx.author();
        let tx_hash = ctx.tx_hash();
        let fork = ctx.fork();
        let decision = VotingDecision::Nay(tx_hash);

        let vote_against = VotingContext::new(decision, author, tx.cfg_hash);
        vote_against.precheck(fork.as_ref()).map_err(|err| {
            error!("Discarding vote against {:?}: {}", tx, err);
            err
        })?;

        vote_against.save(fork);
        trace!(
            "Put VoteAgainst:{:?} to corresponding cfg votes_by_config_hash table",
            tx
        );

        Ok(())
    }
}

impl_service_dispatcher!(ConfigurationServiceImpl, ConfigurationService);

impl Service for ConfigurationServiceImpl {
    fn initialize(&mut self, mut ctx: TransactionContext, arg: Any) -> Result<(), ExecutionError> {
        let arg: ConfigurationServiceInit = BinaryForm::decode(arg.get_value()).map_err(|e| {
            ExecutionError::with_description(WRONG_ARG_ERROR, format!("Wrong argument: {}", e))
        })?;

        if arg.is_custom_majority_count {
            let fork = ctx.fork();
            // Assuming that Service::initialize is called after genesis block is created.
            let actual_config = CoreSchema::new(&fork).actual_configuration();
            let validators_count = actual_config.validator_keys.len();
            
            let byzantine_majority_count = State::byzantine_majority_count(validators_count);
            if (arg.majority_count as usize) > validators_count || (arg.majority_count as usize) < byzantine_majority_count
            {
                return Err(ServiceError::InvalidMajorityCount {
                    min: byzantine_majority_count,
                    max: validators_count,
                    proposed: arg.majority_count as usize,
                })?;
            }

            self.majority_count = Some(arg.majority_count);
        }

        Ok(())
    }
}
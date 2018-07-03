// Copyright 2018 The Exonum Team
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
    api::{self, ServiceApiBuilder, ServiceApiState},
    blockchain::{Schema as CoreSchema, StoredConfiguration}, crypto::{CryptoHash, Hash},
    helpers::Height, node::TransactionSend, storage::StorageValue,
};

use super::{Propose, ProposeData, Schema, Vote, VoteAgainst, VotingDecision};

pub type VotesInfo = Option<Vec<Option<VotingDecision>>>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConfigHashInfo {
    pub hash: Hash,
    pub config: StoredConfiguration,
    pub propose: Option<Hash>,
    pub votes: VotesInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProposeHashInfo {
    pub hash: Hash,
    pub propose_data: ProposeData,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConfigInfo {
    pub committed_config: Option<StoredConfiguration>,
    pub propose: Option<ProposeData>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProposeResponse {
    pub tx_hash: Hash,
    pub cfg_hash: Hash,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VoteResponse {
    pub tx_hash: Hash,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct HashQuery {
    pub hash: Hash,
}

/// Filter for stored configurations.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct FilterQuery {
    pub previous_cfg_hash: Option<Hash>,
    pub actual_from: Option<Height>,
}

#[derive(Debug, Clone, Copy)]
pub struct PrivateApi;

#[derive(Debug, Clone, Copy)]
pub struct PublicApi;

impl FilterQuery {
    /// Checks if a supplied configuration satisfies this filter.
    fn matches(&self, cfg: &StoredConfiguration) -> bool {
        if let Some(ref prev) = self.previous_cfg_hash {
            if cfg.previous_cfg_hash != *prev {
                return false;
            }
        }

        if let Some(from_height) = self.actual_from {
            if cfg.actual_from < from_height {
                return false;
            }
        }

        true
    }
}

impl PublicApi {
    fn config_with_proofs(state: &ServiceApiState, config: StoredConfiguration) -> ConfigHashInfo {
        let propose = Schema::new(state.snapshot())
            .propose(&config.hash())
            .map(|p| p.hash());
        let votes = Self::votes_for_propose(state, &config.hash());
        ConfigHashInfo {
            hash: config.hash(),
            config,
            propose,
            votes,
        }
    }

    fn votes_for_propose(state: &ServiceApiState, config_hash: &Hash) -> VotesInfo {
        let schema = Schema::new(state.snapshot());
        if schema.propose_data_by_config_hash().contains(config_hash) {
            Some(schema.votes(config_hash))
        } else {
            None
        }
    }

    #[cfg_attr(feature = "cargo-clippy", allow(let_and_return))]
    fn proposed_configs(state: &ServiceApiState, filter: &FilterQuery) -> Vec<ProposeHashInfo> {
        let schema = Schema::new(state.snapshot());
        let index = schema.config_hash_by_ordinal();
        let proposes_by_hash = schema.propose_data_by_config_hash();

        let proposes = index
            .iter()
            .map(|cfg_hash| {
                let propose_data = proposes_by_hash.get(&cfg_hash).unwrap_or_else(|| {
                    panic!("Not found propose for following cfg_hash: {:?}", cfg_hash)
                });

                (cfg_hash, propose_data)
            })
            .filter(|&(_, ref propose_data)| {
                let cfg = <StoredConfiguration as StorageValue>::from_bytes(
                    propose_data.tx_propose().cfg().as_bytes().into(),
                );
                filter.matches(&cfg)
            })
            .map(|(hash, propose_data)| ProposeHashInfo { hash, propose_data })
            .collect();
        proposes
    }

    #[cfg_attr(feature = "cargo-clippy", allow(let_and_return))]
    fn committed_configs(state: &ServiceApiState, filter: &FilterQuery) -> Vec<ConfigHashInfo> {
        let core_schema = CoreSchema::new(state.snapshot());
        let actual_from = core_schema.configs_actual_from();
        let configs = core_schema.configs();

        let committed_configs = actual_from
            .iter()
            .map(|config_ref| {
                let config_hash = config_ref.cfg_hash();
                configs.get(config_hash).unwrap_or_else(|| {
                    panic!(
                        "Config with hash {:?} is absent in configs table",
                        config_hash
                    )
                })
            })
            .filter(|config| filter.matches(config))
            .map(|config| Self::config_with_proofs(state, config))
            .collect();
        committed_configs
    }

    fn handle_actual_config(state: &ServiceApiState, _query: ()) -> api::Result<ConfigHashInfo> {
        let config = CoreSchema::new(state.snapshot()).actual_configuration();
        Ok(Self::config_with_proofs(state, config))
    }

    fn handle_following_config(
        state: &ServiceApiState,
        _query: (),
    ) -> api::Result<Option<ConfigHashInfo>> {
        Ok(CoreSchema::new(state.snapshot())
            .following_configuration()
            .map(|cfg| Self::config_with_proofs(state, cfg)))
    }

    fn handle_config_by_hash(state: &ServiceApiState, query: HashQuery) -> api::Result<ConfigInfo> {
        let snapshot = state.snapshot();

        let committed_config = CoreSchema::new(&snapshot).configs().get(&query.hash);
        let propose = Schema::new(&snapshot)
            .propose_data_by_config_hash()
            .get(&query.hash);

        Ok(ConfigInfo {
            committed_config,
            propose,
        })
    }

    fn handle_votes_for_propose(
        state: &ServiceApiState,
        query: HashQuery,
    ) -> api::Result<VotesInfo> {
        Ok(Self::votes_for_propose(state, &query.hash))
    }

    fn handle_proposed_configs(
        state: &ServiceApiState,
        query: FilterQuery,
    ) -> api::Result<Vec<ProposeHashInfo>> {
        Ok(Self::proposed_configs(state, &query))
    }

    fn handle_committed_configs(
        state: &ServiceApiState,
        query: FilterQuery,
    ) -> api::Result<Vec<ConfigHashInfo>> {
        Ok(Self::committed_configs(state, &query))
    }

    pub fn wire(builder: &mut ServiceApiBuilder) {
        builder
            .public_scope()
            .endpoint("v1/configs/actual", Self::handle_actual_config)
            .endpoint("v1/configs/following", Self::handle_following_config)
            .endpoint("v1/configs", Self::handle_config_by_hash)
            .endpoint("v1/configs/votes", Self::handle_votes_for_propose)
            .endpoint("v1/configs/proposed", Self::handle_proposed_configs)
            .endpoint("v1/configs/committed", Self::handle_committed_configs);
    }
}

impl PrivateApi {
    fn handle_propose(
        state: &ServiceApiState,
        config: StoredConfiguration,
    ) -> api::Result<ProposeResponse> {
        config.consensus.warn_if_nonoptimal();

        let cfg_hash = config.hash();
        let propose = Propose::new(
            state.public_key(),
            ::std::str::from_utf8(config.into_bytes().as_slice()).unwrap(),
            state.secret_key(),
        );
        let tx_hash = propose.hash();

        state.sender().send(propose.into())?;

        Ok(ProposeResponse { tx_hash, cfg_hash })
    }

    fn handle_vote(state: &ServiceApiState, query: HashQuery) -> api::Result<VoteResponse> {
        let vote = Vote::new(state.public_key(), &query.hash, state.secret_key());
        let tx_hash = vote.hash();

        state.sender().send(vote.into())?;

        Ok(VoteResponse { tx_hash })
    }

    fn handle_vote_against(state: &ServiceApiState, query: HashQuery) -> api::Result<VoteResponse> {
        let vote_against = VoteAgainst::new(state.public_key(), &query.hash, state.secret_key());
        let tx_hash = vote_against.hash();

        state.sender().send(vote_against.into())?;

        Ok(VoteResponse { tx_hash })
    }

    pub fn wire(builder: &mut ServiceApiBuilder) {
        builder
            .private_scope()
            .endpoint_mut("v1/configs/postpropose", Self::handle_propose)
            .endpoint_mut("v1/configs/postvote", Self::handle_vote)
            .endpoint_mut("v1/configs/postagainst", Self::handle_vote_against);
    }
}

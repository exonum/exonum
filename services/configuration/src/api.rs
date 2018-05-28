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

use bodyparser;
use exonum::api::{Api, ApiError};
use exonum::blockchain::{ApiContext, Blockchain, Schema as CoreSchema, StoredConfiguration};
use exonum::crypto::{CryptoHash, Hash, PublicKey, SecretKey};
use exonum::encoding::serialize::json::reexport as serde_json;
use exonum::helpers::Height;
use exonum::node::{ApiSender, TransactionSend};
use exonum::storage::StorageValue;
use iron::prelude::*;
use router::Router;

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

#[derive(Clone)]
pub struct PrivateApi {
    channel: ApiSender,
    service_keys: (PublicKey, SecretKey),
}

#[derive(Clone)]
pub struct PublicApi {
    blockchain: Blockchain,
}

/// Filter for stored configurations.
struct Filter {
    previous_cfg_hash: Option<Hash>,
    actual_from: Option<Height>,
}

impl Filter {
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
    pub fn new(context: &ApiContext) -> Self {
        PublicApi {
            blockchain: context.blockchain().clone(),
        }
    }

    fn config_with_proofs(&self, config: StoredConfiguration) -> ConfigHashInfo {
        let propose = Schema::new(self.blockchain.snapshot())
            .propose(&config.hash())
            .map(|p| p.hash());
        let votes = self.votes_for_propose(&config.hash());
        ConfigHashInfo {
            hash: config.hash(),
            config,
            propose,
            votes,
        }
    }

    fn votes_for_propose(&self, config_hash: &Hash) -> VotesInfo {
        let schema = Schema::new(self.blockchain.snapshot());
        if schema.propose_data_by_config_hash().contains(config_hash) {
            Some(schema.votes(config_hash))
        } else {
            None
        }
    }

    #[cfg_attr(feature = "cargo-clippy", allow(let_and_return))]
    fn proposed_configs(&self, filter: &Filter) -> Vec<ProposeHashInfo> {
        let schema = Schema::new(self.blockchain.snapshot());
        let index = schema.config_hash_by_ordinal();
        let proposes_by_hash = schema.propose_data_by_config_hash();

        let proposes = index
            .iter()
            .map(|cfg_hash| {
                let propose_data = proposes_by_hash.get(&cfg_hash).expect(&format!(
                    "Not found propose for following cfg_hash: {:?}",
                    cfg_hash
                ));

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
    fn committed_configs(&self, filter: &Filter) -> Vec<ConfigHashInfo> {
        let core_schema = CoreSchema::new(self.blockchain.snapshot());
        let actual_from = core_schema.configs_actual_from();
        let configs = core_schema.configs();

        let committed_configs = actual_from
            .iter()
            .map(|config_ref| {
                let config_hash = config_ref.cfg_hash();
                configs.get(config_hash).expect(&format!(
                    "Config with hash {:?} is absent in configs table",
                    config_hash
                ))
            })
            .filter(|config| filter.matches(config))
            .map(|config| self.config_with_proofs(config))
            .collect();
        committed_configs
    }

    fn handle_actual_config(self, router: &mut Router) {
        let actual_config = move |_: &mut Request| -> IronResult<Response> {
            let config = CoreSchema::new(self.blockchain.snapshot()).actual_configuration();
            let config = self.config_with_proofs(config);
            self.ok_response(&serde_json::to_value(config).unwrap())
        };

        router.get("/v1/configs/actual", actual_config, "actual_config");
    }

    fn handle_following_config(self, router: &mut Router) {
        let following_config = move |_: &mut Request| -> IronResult<Response> {
            let config = CoreSchema::new(self.blockchain.snapshot())
                .following_configuration()
                .map(|cfg| self.config_with_proofs(cfg));
            self.ok_response(&serde_json::to_value(config).unwrap())
        };

        router.get(
            "/v1/configs/following",
            following_config,
            "following_config",
        );
    }

    fn handle_config_by_hash(self, router: &mut Router) {
        let config_by_hash = move |req: &mut Request| -> IronResult<Response> {
            let hash: Hash = self.url_fragment(req, "hash")?;

            let snapshot = self.blockchain.snapshot();
            let committed_config = CoreSchema::new(&snapshot).configs().get(&hash);
            let propose = Schema::new(&snapshot)
                .propose_data_by_config_hash()
                .get(&hash);

            self.ok_response(&serde_json::to_value(ConfigInfo {
                committed_config,
                propose,
            }).unwrap())
        };

        router.get("/v1/configs/:hash", config_by_hash, "config_by_hash");
    }

    fn handle_votes_for_propose(self, router: &mut Router) {
        let votes_for_propose = move |req: &mut Request| -> IronResult<Response> {
            let config_hash: Hash = self.url_fragment(req, "hash")?;
            let votes = self.votes_for_propose(&config_hash);
            self.ok_response(&serde_json::to_value(votes).unwrap())
        };

        router.get(
            "/v1/configs/:hash/votes",
            votes_for_propose,
            "votes_for_propose",
        );
    }

    fn retrieve_filter(&self, request: &mut Request) -> Result<Filter, ApiError> {
        let previous_cfg_hash: Option<Hash> = self.optional_param(request, "previous_cfg_hash")?;
        let actual_from: Option<Height> = self.optional_param(request, "actual_from")?;
        Ok(Filter {
            previous_cfg_hash,
            actual_from,
        })
    }

    fn handle_proposed_configs(self, router: &mut Router) {
        let proposed_configs = move |req: &mut Request| -> IronResult<Response> {
            let proposes = self.proposed_configs(&self.retrieve_filter(req)?);
            self.ok_response(&serde_json::to_value(proposes).unwrap())
        };

        router.get("/v1/configs/proposed", proposed_configs, "proposed_configs");
    }

    fn handle_committed_configs(self, router: &mut Router) {
        let committed_configs = move |req: &mut Request| -> IronResult<Response> {
            let configs = self.committed_configs(&self.retrieve_filter(req)?);
            self.ok_response(&serde_json::to_value(configs).unwrap())
        };

        router.get(
            "/v1/configs/committed",
            committed_configs,
            "committed_configs",
        );
    }
}

impl PrivateApi {
    pub fn new(context: &ApiContext) -> Self {
        PrivateApi {
            channel: context.node_channel().clone(),
            service_keys: (*context.public_key(), context.secret_key().clone()),
        }
    }

    fn handle_propose(self, router: &mut Router) {
        let post_propose = move |req: &mut Request| -> IronResult<Response> {
            let config = match req.get::<bodyparser::Struct<StoredConfiguration>>() {
                Ok(Some(config)) => config,
                Ok(None) => Err(ApiError::BadRequest("Empty request body".into()))?,
                Err(e) => Err(ApiError::BadRequest(e.to_string()))?,
            };

            config.consensus.warn_if_nonoptimal();

            let cfg_hash = config.hash();
            let propose = Propose::new(
                &self.service_keys.0,
                ::std::str::from_utf8(config.into_bytes().as_slice()).unwrap(),
                &self.service_keys.1,
            );
            let tx_hash = propose.hash();

            self.channel.send(propose.into()).map_err(ApiError::from)?;

            let response = ProposeResponse { tx_hash, cfg_hash };
            self.ok_response(&serde_json::to_value(response).unwrap())
        };

        router.post("/v1/configs/postpropose", post_propose, "post_propose");
    }

    fn handle_vote(self, router: &mut Router) {
        let post_vote = move |req: &mut Request| -> IronResult<Response> {
            let cfg_hash: Hash = self.url_fragment(req, "hash")?;

            let vote = Vote::new(&self.service_keys.0, &cfg_hash, &self.service_keys.1);
            let tx_hash = vote.hash();

            self.channel.send(vote.into()).map_err(ApiError::from)?;

            let response = VoteResponse { tx_hash };
            self.ok_response(&serde_json::to_value(response).unwrap())
        };

        router.post("/v1/configs/:hash/postvote", post_vote, "post_vote");
    }

    fn handle_vote_against(self, router: &mut Router) {
        let post_vote_against = move |req: &mut Request| -> IronResult<Response> {
            let cfg_hash: Hash = self.url_fragment(req, "hash")?;

            let vote_against =
                VoteAgainst::new(&self.service_keys.0, &cfg_hash, &self.service_keys.1);
            let tx_hash = vote_against.hash();

            self.channel
                .send(vote_against.into())
                .map_err(ApiError::from)?;

            let response = VoteResponse { tx_hash };
            self.ok_response(&serde_json::to_value(response).unwrap())
        };

        router.post(
            "/v1/configs/:hash/postagainst",
            post_vote_against,
            "post_vote_against",
        );
    }
}

impl Api for PublicApi {
    fn wire(&self, router: &mut Router) {
        self.clone().handle_actual_config(router);
        self.clone().handle_following_config(router);
        self.clone().handle_config_by_hash(router);
        self.clone().handle_votes_for_propose(router);
        self.clone().handle_proposed_configs(router);
        self.clone().handle_committed_configs(router);
    }
}

impl Api for PrivateApi {
    fn wire(&self, router: &mut Router) {
        self.clone().handle_propose(router);
        self.clone().handle_vote(router);
        self.clone().handle_vote_against(router);
    }
}

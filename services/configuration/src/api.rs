// Copyright 2017 The Exonum Team
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

use std::str;

use router::Router;
use iron::prelude::*;
use bodyparser;
use exonum::api::{Api, ApiError};
use exonum::crypto::{CryptoHash, PublicKey, SecretKey, Hash};
use exonum::blockchain::{Blockchain, StoredConfiguration, Schema};
use exonum::storage::StorageValue;
use exonum::node::TransactionSend;
use exonum::encoding::serialize::json::reexport as serde_json;
use exonum::helpers::Height;

use super::{ProposeData, TxConfigPropose, TxConfigVote, ConfigurationSchema};

pub type VotesInfo = Option<Vec<Option<TxConfigVote>>>;

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
pub struct ProposePost {
    pub tx_hash: Hash,
    pub cfg_hash: Hash,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VotePost {
    pub tx_hash: Hash,
}

#[derive(Clone)]
pub struct PrivateConfigApi<T: TransactionSend + Clone> {
    pub channel: T,
    pub config: (PublicKey, SecretKey),
}

#[derive(Clone)]
pub struct PublicConfigApi {
    pub blockchain: Blockchain,
}

impl PublicConfigApi {
    fn get_config_with_proofs(&self, config: StoredConfiguration) -> ConfigHashInfo {
        let propose = ConfigurationSchema::new(self.blockchain.snapshot())
            .get_propose(&config.hash())
            .map(|p| p.hash());
        let votes = self.get_votes_for_propose(&config.hash());
        ConfigHashInfo {
            hash: config.hash(),
            config,
            propose,
            votes,
        }
    }

    fn get_actual_config(&self) -> ConfigHashInfo {
        let snapshot = self.blockchain.snapshot();
        let configuration_schema = Schema::new(&snapshot);
        let actual_cfg = configuration_schema.actual_configuration();
        self.get_config_with_proofs(actual_cfg)
    }

    fn get_following_config(&self) -> Option<ConfigHashInfo> {
        Schema::new(self.blockchain.snapshot())
            .following_configuration()
            .map(|following_cfg| self.get_config_with_proofs(following_cfg))
    }

    fn get_config_by_hash(&self, hash: &Hash) -> ConfigInfo {
        let snapshot = self.blockchain.snapshot();
        let general_schema = Schema::new(&snapshot);
        let committed_config = general_schema.configs().get(hash);

        let configuration_schema = ConfigurationSchema::new(&snapshot);
        let propose = configuration_schema.propose_data_by_config_hash().get(hash);
        ConfigInfo {
            committed_config,
            propose,
        }
    }

    fn get_votes_for_propose(&self, config_hash: &Hash) -> VotesInfo {
        let snapshot = self.blockchain.snapshot();
        let configuration_schema = ConfigurationSchema::new(&snapshot);
        if configuration_schema.propose_data_by_config_hash().contains(config_hash) {
            Some(configuration_schema.get_votes(config_hash))
        } else {
            None
        }
    }

    fn filter_cfg_predicate(
        cfg: &StoredConfiguration,
        previous_cfg_hash_filter: Option<Hash>,
        actual_from_filter: Option<Height>,
    ) -> bool {
        if let Some(prev_ref) = previous_cfg_hash_filter {
            if cfg.previous_cfg_hash != prev_ref {
                return false;
            }
        }
        if let Some(from_height) = actual_from_filter {
            if cfg.actual_from < from_height {
                return false;
            }
        }
        true
    }

    fn get_all_proposes(
        &self,
        previous_cfg_hash_filter: Option<Hash>,
        actual_from_filter: Option<Height>,
    ) -> Vec<ProposeHashInfo> {
        let snapshot = self.blockchain.snapshot();
        let configuration_schema = ConfigurationSchema::new(&snapshot);
        let index = configuration_schema.config_hash_by_ordinal();
        let proposes = {
            index
                .into_iter()
                .map(|cfg_hash| {
                    let propose_data = configuration_schema
                        .propose_data_by_config_hash()
                        .get(&cfg_hash)
                        .expect(&format!(
                            "Not found propose for following cfg_hash: {:?}",
                            cfg_hash
                        ));

                    (cfg_hash, propose_data)
                })
                .filter(|&(_, ref propose_data)| {
                    let cfg = <StoredConfiguration as StorageValue>::from_bytes(
                        propose_data.tx_propose().cfg().as_bytes().into(),
                    );
                    PublicConfigApi::filter_cfg_predicate(
                        &cfg,
                        previous_cfg_hash_filter,
                        actual_from_filter,
                    )
                })
                .map(|(cfg_hash, propose_data)| {
                    ProposeHashInfo {
                        hash: cfg_hash,
                        propose_data,
                    }
                })
                .collect::<Vec<_>>()
        };
        proposes
    }

    fn get_all_committed(
        &self,
        previous_cfg_hash_filter: Option<Hash>,
        actual_from_filter: Option<Height>,
    ) -> Vec<ConfigHashInfo> {
        let snapshot = self.blockchain.snapshot();
        let general_schema = Schema::new(&snapshot);
        let actual_from = general_schema.configs_actual_from();
        let configs = general_schema.configs();

        let committed_configs = {
            actual_from
                .into_iter()
                .map(|reference| {
                    let config_hash = reference.cfg_hash();
                    configs.get(config_hash).expect(&format!(
                        "Config with hash {:?} is absent in configs table",
                        config_hash
                    ))
                })
                .filter(|config| {
                    PublicConfigApi::filter_cfg_predicate(
                        config,
                        previous_cfg_hash_filter,
                        actual_from_filter,
                    )
                })
                .map(|config| self.get_config_with_proofs(config))
                .collect::<Vec<_>>()
        };
        committed_configs
    }

    fn retrieve_params(
        &self,
        request: &mut Request,
    ) -> Result<(Option<Hash>, Option<Height>), ApiError> {
        let previous_cfg_hash: Option<Hash> = self.optional_param(request, "previous_cfg_hash")?;
        let actual_from: Option<Height> = self.optional_param(request, "actual_from")?;
        Ok((previous_cfg_hash, actual_from))
    }
}

impl<T> PrivateConfigApi<T>
where
    T: TransactionSend + Clone,
{
    fn put_config_propose(
        &self,
        cfg: StoredConfiguration,
    ) -> Result<ProposePost, ApiError> {
        let cfg_hash = cfg.hash();
        let config_propose = TxConfigPropose::new(
            &self.config.0,
            str::from_utf8(cfg.into_bytes().as_slice()).unwrap(),
            &self.config.1,
        );
        let tx_hash = config_propose.hash();
        let ch = self.channel.clone();
        ch.send(Box::new(config_propose))?;
        let res = ProposePost { tx_hash, cfg_hash };
        Ok(res)
    }

    fn put_config_vote(&self, cfg_hash: &Hash) -> Result<VotePost, ApiError> {
        let config_vote = TxConfigVote::new(&self.config.0, cfg_hash, &self.config.1);
        let tx_hash = config_vote.hash();
        let ch = self.channel.clone();
        ch.send(Box::new(config_vote))?;
        let res = VotePost { tx_hash };
        Ok(res)
    }
}

impl Api for PublicConfigApi {
    fn wire(&self, router: &mut Router) {

        let self_ = self.clone();
        let config_actual = move |_: &mut Request| -> IronResult<Response> {
            let info = self_.get_actual_config();
            self_.ok_response(&serde_json::to_value(info).unwrap())
        };

        let self_ = self.clone();
        let config_following = move |_: &mut Request| -> IronResult<Response> {
            let info = self_.get_following_config();
            self_.ok_response(&serde_json::to_value(info).unwrap())
        };

        let self_ = self.clone();
        let config_by_hash = move |req: &mut Request| -> IronResult<Response> {
            let hash: Hash = self_.url_fragment(req, "hash")?;
            let info = self_.get_config_by_hash(&hash);
            self_.ok_response(&serde_json::to_value(info).unwrap())
        };

        let self_ = self.clone();
        let get_votes_for_propose = move |req: &mut Request| -> IronResult<Response> {
            let propose_cfg_hash: Hash = self_.url_fragment(req, "hash")?;
            let info = self_.get_votes_for_propose(&propose_cfg_hash);
            self_.ok_response(&serde_json::to_value(info).unwrap())
        };

        let self_ = self.clone();
        let get_all_proposes = move |req: &mut Request| -> IronResult<Response> {
            let (previous_cfg_hash, actual_from) = self_.retrieve_params(req)?;
            let info = self_.get_all_proposes(previous_cfg_hash, actual_from);
            self_.ok_response(&serde_json::to_value(info).unwrap())
        };

        let self_ = self.clone();
        let get_all_committed = move |req: &mut Request| -> IronResult<Response> {
            let (previous_cfg_hash, actual_from) = self_.retrieve_params(req)?;
            let info = self_.get_all_committed(previous_cfg_hash, actual_from);
            self_.ok_response(&serde_json::to_value(info).unwrap())
        };
        router.get("/v1/configs/actual", config_actual, "config_actual");
        router.get(
            "/v1/configs/following",
            config_following,
            "config_following",
        );
        router.get("/v1/configs/:hash", config_by_hash, "config_by_hash");
        router.get(
            "/v1/configs/:hash/votes",
            get_votes_for_propose,
            "get_votes_for_propose",
        );
        router.get("/v1/configs/proposed", get_all_proposes, "get_all_proposes");
        router.get(
            "/v1/configs/committed",
            get_all_committed,
            "get_all_committed",
        );

    }
}

impl<T> Api for PrivateConfigApi<T>
where
    T: 'static + TransactionSend + Clone,
{
    fn wire(&self, router: &mut Router) {
        let self_ = self.clone();
        let put_config_propose = move |req: &mut Request| -> IronResult<Response> {
            match req.get::<bodyparser::Struct<StoredConfiguration>>() {
                Ok(Some(cfg)) => {
                    let info = self_.put_config_propose(cfg)?;
                    self_.ok_response(&serde_json::to_value(info).unwrap())
                }
                Ok(None) => Err(ApiError::BadRequest("Empty request body".into()))?,
                Err(e) => Err(ApiError::BadRequest(e.to_string()))?,
            }
        };

        let self_ = self.clone();
        let put_config_vote = move |req: &mut Request| -> IronResult<Response> {
            let propose_cfg_hash = self_.url_fragment(req, "hash")?;
            let info = self_.put_config_vote(&propose_cfg_hash)?;
            self_.ok_response(&serde_json::to_value(info).unwrap())
        };
        router.post(
            "/v1/configs/postpropose",
            put_config_propose,
            "put_config_propose",
        );
        router.post(
            "/v1/configs/:hash/postvote",
            put_config_vote,
            "put_config_vote",
        );
    }
}

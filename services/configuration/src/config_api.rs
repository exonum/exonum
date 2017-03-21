use serde_json::value::ToJson;
use router::Router;
use blockchain_explorer::api::{Api, ApiError};
use iron::prelude::*;
use bodyparser;
use exonum::crypto::{Hash, HexValue};
use exonum::blockchain::{Blockchain, StoredConfiguration, Schema};
use ::{ConfigVotingData, TxConfigPropose, TxConfigVote, ConfigTx, ConfigurationSchema};
use exonum::storage::{Map, StorageValue};

use exonum::node::{TxSender, NodeChannel, NodeConfig, TransactionSend};
pub type ConfigTxSender = TxSender<NodeChannel>;

#[derive(Serialize, Deserialize)]
pub struct ConfigWithHash {
    hash: Hash,
    config: StoredConfiguration,
}

#[derive(Serialize)]
pub struct ConfigInfo {
    committed_config: Option<StoredConfiguration>,
    propose: Option<ConfigVotingData>,
}

#[derive(Serialize)]
pub enum ConfigVotesInfo {
    Votes(Vec<Option<TxConfigVote>>),
    ProposeAbsent(Option<()>),
}

#[derive(Serialize, Deserialize)]
pub struct ProposeRequestResponse {
    tx_hash: Hash,
    cfg_hash: Hash,
}

#[derive(Serialize, Deserialize)]
pub struct VoteRequestResponse {
    tx_hash: Hash,
}

#[derive(Clone)]
pub struct ConfigApi<T: TransactionSend + Clone> {
    pub blockchain: Blockchain,
    pub channel: T,
    pub config: NodeConfig,
}

impl<T> ConfigApi<T>
    where T: TransactionSend + Clone
{
    fn get_actual_config(&self) -> Result<ConfigWithHash, ApiError> {
        let actual_cfg = Schema::new(&self.blockchain.view()).get_actual_configuration()?;
        let res = ConfigWithHash {
            hash: actual_cfg.hash(),
            config: actual_cfg,
        };
        Ok(res)
    }

    fn get_following_config(&self) -> Result<Option<ConfigWithHash>, ApiError> {
        let following_cfg = Schema::new(&self.blockchain.view()).get_following_configuration()?;
        let res = following_cfg.map(|cfg| {
            ConfigWithHash {
                hash: cfg.hash(),
                config: cfg,
            }
        });
        Ok(res)
    }

    fn get_config_by_hash(&self, hash: &Hash) -> Result<ConfigInfo, ApiError> {
        let view = self.blockchain.view();
        let general_schema = Schema::new(&view);
        let committed_config = general_schema.configs().get(hash)?;

        let configuration_schema = ConfigurationSchema::new(&view);
        let propose = configuration_schema.config_data().get(hash)?;
        let res = ConfigInfo {
            committed_config: committed_config,
            propose: propose,
        };
        Ok(res)
    }

    fn put_config_propose(&self,
                          cfg: StoredConfiguration)
                          -> Result<ProposeRequestResponse, ApiError> {
        let cfg_hash = cfg.hash();
        let config_propose = TxConfigPropose::new(&self.config.public_key,
                                                  &cfg.serialize(),
                                                  &self.config.secret_key);
        let tx_hash = config_propose.hash();
        let ch = self.channel.clone();
        ch.send(ConfigTx::ConfigPropose(config_propose))?;
        let res = ProposeRequestResponse {
            tx_hash: tx_hash,
            cfg_hash: cfg_hash,
        };
        Ok(res)
    }

    fn put_config_vote(&self, cfg_hash: &Hash) -> Result<VoteRequestResponse, ApiError> {
        let config_vote =
            TxConfigVote::new(&self.config.public_key, cfg_hash, &self.config.secret_key);
        let tx_hash = config_vote.hash();
        let ch = self.channel.clone();
        ch.send(ConfigTx::ConfigVote(config_vote))?;
        let res = VoteRequestResponse { tx_hash: tx_hash };
        Ok(res)
    }

    fn get_votes_for_propose(&self, cfg_hash: &Hash) -> Result<ConfigVotesInfo, ApiError> {
        let view = self.blockchain.view();
        let configuration_schema = ConfigurationSchema::new(&view);
        let res = match configuration_schema.config_data().get(cfg_hash)? {
            None => ConfigVotesInfo::ProposeAbsent(None), 
            Some(_) => {
                ConfigVotesInfo::Votes(configuration_schema.get_votes(cfg_hash)?)
            }
        };
        Ok(res)
    }
}

impl<T> Api for ConfigApi<T>
    where T: 'static + TransactionSend + Clone
{
    fn wire(&self, router: &mut Router) {

        let _self = self.clone();
        let config_actual = move |_: &mut Request| -> IronResult<Response> {
            let info = _self.get_actual_config()?;
            _self.ok_response(&info.to_json())
        };

        let _self = self.clone();
        let config_following = move |_: &mut Request| -> IronResult<Response> {
            let info = _self.get_following_config()?;
            _self.ok_response(&info.to_json())
        };

        let _self = self.clone();
        let config_by_hash = move |req: &mut Request| -> IronResult<Response> {
            let params = req.extensions.get::<Router>().unwrap();
            match params.find("hash") {
                Some(hash_str) => {
                    let hash = Hash::from_hex(hash_str).map_err(ApiError::from)?;
                    let info = _self.get_config_by_hash(&hash)?;
                    _self.ok_response(&info.to_json())
                } 
                None => return Err(ApiError::IncorrectRequest)?, 
            }
        };

        let _self = self.clone();
        let put_config_propose = move |req: &mut Request| -> IronResult<Response> {
            match req.get::<bodyparser::Struct<StoredConfiguration>>() {
                Ok(Some(cfg)) => {
                    let info = _self.put_config_propose(cfg)?;
                    _self.ok_response(&info.to_json())
                }
                Ok(None) => Err(ApiError::IncorrectRequest)?,
                Err(e) => {
                    error!("Couldn't parse stored configurations:{:?}", e);
                    Err(ApiError::IncorrectRequest)?
                }
            }
        };

        let _self = self.clone();
        let put_config_vote = move |req: &mut Request| -> IronResult<Response> {
            let params = req.extensions.get::<Router>().unwrap();
            match params.find("hash") {
                Some(hash_str) => {
                    let propose_cfg_hash = Hash::from_hex(hash_str).map_err(ApiError::from)?;
                    let info = _self.put_config_vote(&propose_cfg_hash)?;
                    _self.ok_response(&info.to_json())
                } 
                None => return Err(ApiError::IncorrectRequest)?, 
            }
        };

        let _self = self.clone();
        let get_votes_for_propose = move |req: &mut Request| -> IronResult<Response> {
            let params = req.extensions.get::<Router>().unwrap();
            match params.find("hash") {
                Some(hash_str) => {
                    let propose_cfg_hash = Hash::from_hex(hash_str).map_err(ApiError::from)?;
                    let info = _self.get_votes_for_propose(&propose_cfg_hash)?;
                    _self.ok_response(&info.to_json())
                } 
                None => return Err(ApiError::IncorrectRequest)?, 
            }
        };
        router.get("/api/v1/config/actual", config_actual, "config_actual");
        router.get("/api/v1/config/following",
                   config_following,
                   "config_following");
        router.get("/api/v1/configs/:hash", config_by_hash, "config_by_hash");
        router.get("/api/v1/configs/:hash/votes",
                   get_votes_for_propose,
                   "get_votes_for_propose");

        router.post("/api/v1/configs/postpropose",
                    put_config_propose,
                    "put_config_propose");
        router.post("/api/v1/configs/:hash/postvote",
                    put_config_vote,
                    "put_config_vote");
    }
}

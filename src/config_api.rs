use serde_json::value::ToJson;
use router::Router;
use blockchain_explorer::api::{Api, ApiError};
use iron::prelude::*;
use bodyparser;
use exonum::crypto::{PublicKey, SecretKey, Hash, HexValue};
use exonum::blockchain::{Blockchain, StoredConfiguration, Schema};
use {StorageValueConfigProposeData, TxConfigPropose, TxConfigVote, ConfigTx, ConfigurationSchema};
use exonum::storage::{Map, StorageValue};

use exonum::node::{TxSender, NodeChannel, TransactionSend};
pub type ConfigTxSender = TxSender<NodeChannel>;

#[derive(Serialize, Deserialize)]
pub struct ApiResponseConfigHashInfo {
    pub hash: Hash,
    pub config: StoredConfiguration,
}

#[derive(Serialize, Deserialize)]
pub struct ApiResponseConfigInfo {
    pub committed_config: Option<StoredConfiguration>,
    pub propose: Option<StorageValueConfigProposeData>,
}

#[derive(Serialize, Deserialize)]
pub enum ApiResponseVotesInfo {
    Votes(Vec<Option<TxConfigVote>>),
    ProposeAbsent(Option<()>),
}

#[derive(Serialize, Deserialize)]
pub struct ApiResponseProposePost {
    pub tx_hash: Hash,
    pub cfg_hash: Hash,
}

#[derive(Serialize, Deserialize)]
pub struct ApiResponseVotePost {
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
    fn get_actual_config(&self) -> Result<ApiResponseConfigHashInfo, ApiError> {
        let actual_cfg = Schema::new(&self.blockchain.view())
            .get_actual_configuration()?;
        let res = ApiResponseConfigHashInfo {
            hash: actual_cfg.hash(),
            config: actual_cfg,
        };
        Ok(res)
    }

    fn get_following_config(&self) -> Result<Option<ApiResponseConfigHashInfo>, ApiError> {
        let following_cfg = Schema::new(&self.blockchain.view())
            .get_following_configuration()?;
        let res = following_cfg.map(|cfg| {
                                        ApiResponseConfigHashInfo {
                                            hash: cfg.hash(),
                                            config: cfg,
                                        }
                                    });
        Ok(res)
    }

    fn get_config_by_hash(&self, hash: &Hash) -> Result<ApiResponseConfigInfo, ApiError> {
        let view = self.blockchain.view();
        let general_schema = Schema::new(&view);
        let committed_config = general_schema.configs().get(hash)?;

        let configuration_schema = ConfigurationSchema::new(&view);
        let propose = configuration_schema.config_data().get(hash)?;
        let res = ApiResponseConfigInfo {
            committed_config: committed_config,
            propose: propose,
        };
        Ok(res)
    }


    fn get_votes_for_propose(&self, cfg_hash: &Hash) -> Result<ApiResponseVotesInfo, ApiError> {
        let view = self.blockchain.view();
        let configuration_schema = ConfigurationSchema::new(&view);
        let res = match configuration_schema.config_data().get(cfg_hash)? {
            None => ApiResponseVotesInfo::ProposeAbsent(None),
            Some(_) => ApiResponseVotesInfo::Votes(configuration_schema.get_votes(cfg_hash)?),
        };
        Ok(res)
    }
}

impl<T> PrivateConfigApi<T>
    where T: TransactionSend + Clone
{
    fn put_config_propose(&self,
                          cfg: StoredConfiguration)
                          -> Result<ApiResponseProposePost, ApiError> {
        let cfg_hash = cfg.hash();
        let config_propose = TxConfigPropose::new(&self.config.0, &cfg.serialize(), &self.config.1);
        let tx_hash = config_propose.hash();
        let ch = self.channel.clone();
        ch.send(ConfigTx::ConfigPropose(config_propose))?;
        let res = ApiResponseProposePost {
            tx_hash: tx_hash,
            cfg_hash: cfg_hash,
        };
        Ok(res)
    }

    fn put_config_vote(&self, cfg_hash: &Hash) -> Result<ApiResponseVotePost, ApiError> {
        let config_vote = TxConfigVote::new(&self.config.0, cfg_hash, &self.config.1);
        let tx_hash = config_vote.hash();
        let ch = self.channel.clone();
        ch.send(ConfigTx::ConfigVote(config_vote))?;
        let res = ApiResponseVotePost { tx_hash: tx_hash };
        Ok(res)
    }
}

impl Api for PublicConfigApi {
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
                None => Err(ApiError::IncorrectRequest)?,
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
                None => Err(ApiError::IncorrectRequest)?,
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

    }
}

impl<T> Api for PrivateConfigApi<T>
    where T: 'static + TransactionSend + Clone
{
    fn wire(&self, router: &mut Router) {
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
                None => Err(ApiError::IncorrectRequest)?,
            }
        };
        router.post("/api/v1/configs/postpropose",
                    put_config_propose,
                    "put_config_propose");
        router.post("/api/v1/configs/:hash/postvote",
                    put_config_vote,
                    "put_config_vote");
    }
}

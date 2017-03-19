use serde_json::value::ToJson;
use params::{Params, Value};
use router::Router;
use blockchain_explorer::api::{Api, ApiError};
use iron::prelude::*;
use bodyparser;
use exonum::crypto::{PublicKey, Hash, HexValue};
use exonum::blockchain::{Blockchain, ConsensusConfig, StoredConfiguration, Schema};
use ::{TxConfigPropose, TxConfigVote, ConfigTx, ConfigurationSchema};
use exonum::storage::{Map, StorageValue, Error as StorageError};

use exonum::node::{TxSender, NodeChannel, NodeConfig, TransactionSend};
pub type ConfigTxSender = TxSender<NodeChannel>;

#[derive(Serialize)]
struct TxHash {
    pub tx_hash: String,
}

#[derive(Serialize)]
struct ConfigProposeInfo {
    from: PublicKey,
    config: StoredConfiguration,
}

#[derive(Clone, Deserialize)]
struct ConfigProposeRequest {
    pub previous_cfg_hash: Hash, 
    pub config: StoredConfiguration, 
}

#[derive(Clone, Deserialize)]
struct ConfigVoteRequest {
    pub hash_propose: Hash,
}

#[derive(Clone)]
pub struct ConfigApi {
    pub blockchain: Blockchain,
    pub channel: ConfigTxSender,
    pub config: NodeConfig,
}

impl ConfigApi {
    fn get_actual_config(&self) -> Result<StoredConfiguration, ApiError> {
        match Schema::new(&self.blockchain.view()).get_actual_configuration() {
            Ok(config) => Ok(config),
            Err(e) => Err(ApiError::Storage(e))?,
        }
    }

    fn get_propose_by_hash(&self,
                           hash_str: &String)
                           -> Result<Option<ConfigProposeInfo>, ApiError> {
        let hash = Hash::from_hex(hash_str)?;
        if let Some(propose) =
               ConfigurationSchema::new(&self.blockchain.view()).config_proposes().get(&hash)? {
                unimplemented!();
        }
        Ok(None)
    }

    fn get_vote_by_key(&self, pub_key_str: &String) -> Result<Option<TxConfigVote>, ApiError> {
        unimplemented!();
        //let pub_key = PublicKey::from_hex(pub_key_str)?;
        //match ConfigurationSchema::new(&self.blockchain.view()).get_vote(&pub_key) {
            //Ok(vote) => Ok(vote),
            //Err(e) => Err(ApiError::Storage(e)),
        //}
    }

    fn put_config_propose(&self, request: ConfigProposeRequest) -> Result<Hash, ApiError> {
        let config_propose = TxConfigPropose::new(&self.config.public_key,
                                                  &request.previous_cfg_hash, 
                                                  request.config.serialize().as_slice(),
                                                  &self.config.secret_key);
        let hash = config_propose.hash();
        match self.channel.send(ConfigTx::ConfigPropose(config_propose)) {
            Ok(_) => Ok(hash),
            Err(e) => Err(ApiError::Events(e)),
        }

    }

    fn put_vote(&self, request: ConfigVoteRequest) -> Result<Hash, ApiError> {
        let height = Schema::new(&self.blockchain.view()).last_height().unwrap();
        let config_vote = TxConfigVote::new(&self.config.public_key,
                                            &request.hash_propose,
                                            &self.config.secret_key);
        let hash = config_vote.hash();
        match self.channel.send(ConfigTx::ConfigVote(config_vote)) {
            Ok(_) => Ok(hash),
            Err(e) => Err(ApiError::Events(e)),
        }
    }
}

impl Api for ConfigApi {
    fn wire(&self, router: &mut Router) {

        let _self = self.clone();
        let configs_actual = move |_: &mut Request| -> IronResult<Response> {
            let info = _self.get_actual_config()?;
            _self.ok_response(&info.to_json())
        };

        let _self = self.clone();
        let configs_propose_by_hash = move |req: &mut Request| -> IronResult<Response> {
            let map = req.get_ref::<Params>().unwrap();
            match map.find(&["hash"]) {
                Some(&Value::String(ref hash)) => {
                    match _self.get_propose_by_hash(hash)? {
                        Some(propose) => _self.ok_response(&propose.to_json()),
                        None => Err(ApiError::NotFound)?,
                    }
                }
                _ => return Err(ApiError::IncorrectRequest)?,
            }
        };

        let _self = self.clone();
        let configs_vote_by_key = move |req: &mut Request| -> IronResult<Response> {
            let map = req.get_ref::<Params>().unwrap();
            match map.find(&["from"]) {
                Some(&Value::String(ref pub_key)) => {
                    match _self.get_vote_by_key(pub_key)? {
                        Some(vote) => _self.ok_response(&vote.to_json()),
                        None => Err(ApiError::NotFound)?,
                    }
                }
                _ => return Err(ApiError::IncorrectRequest)?,
            }
        };

        let _self = self.clone();
        let configs_propose = move |req: &mut Request| -> IronResult<Response> {
            match req.get::<bodyparser::Struct<ConfigProposeRequest>>().unwrap() {
                Some(request) => {
                    let hash = _self.put_config_propose(request)?;
                    let result = TxHash { tx_hash: hash.to_hex() };
                    _self.ok_response(&result.to_json())
                }
                None => Err(ApiError::IncorrectRequest)?,
            }
        };

        let _self = self.clone();
        let configs_vote = move |req: &mut Request| -> IronResult<Response> {
            match req.get::<bodyparser::Struct<ConfigVoteRequest>>().unwrap() {
                Some(request) => {
                    let hash = _self.put_vote(request)?;
                    let result = TxHash { tx_hash: hash.to_hex() };
                    _self.ok_response(&result.to_json())
                }
                None => Err(ApiError::IncorrectRequest)?,
            }
        };

        router.get("/v1/api/configs/actual", configs_actual, "configs_actual");
        router.get("/v1/api/configs/propose/:hash",
                   configs_propose_by_hash,
                   "hash");
        router.get("/v1/api/configs/vote/:from", configs_vote_by_key, "from");
        router.put("/v1/api/configs/propose",
                   configs_propose,
                   "configs_propose");
        router.put("/v1/api/configs/vote", configs_vote, "configs_vote");

    }
}

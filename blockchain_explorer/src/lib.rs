mod explorer;
pub mod helpers;

extern crate time;
extern crate serde;
extern crate jsonway;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate exonum;
extern crate rustless;
extern crate valico;

extern crate clap;
extern crate env_logger;
extern crate log;
extern crate term;
extern crate colored;

use std::ops::Deref;
use std::marker::PhantomData;
use std::error;
use std::fmt;
use serde_json::value::from_value;

use serde::{Serialize, Serializer};
use serde::de;
use serde::de::{Visitor, Deserialize, Deserializer};
use rustless::json::ToJson;
use rustless::{Api, Nesting};
use valico::json_dsl;

use exonum::crypto::{Hash, PublicKey, SecretKey, HexValue, ToHex};
use exonum::storage::Error as StorageError;
use exonum::messages::RawTransaction;
use exonum::blockchain::{Blockchain, Schema};
use exonum::services::configuration::ConfigurationSchema;
use exonum::node::{Node, NodeConfig, NodeChannel, TxSender};
use exonum::services::configuration::{ConfigTx, TxConfigPropose, TxConfigVote};

pub use explorer::{TransactionInfo, BlockchainExplorer, BlockInfo};

pub type Channel = TxSender<NodeChannel>;

#[derive(Debug)]
pub struct ValueNotFound(String);

impl fmt::Display for ValueNotFound {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ValueNotFound: {}", self.0)
    }
}

impl error::Error for ValueNotFound {
    fn description(&self) -> &str {
        self.0.as_ref()
    }
}

impl ValueNotFound {
    pub fn new<T: Into<String>>(s: T) -> ValueNotFound {
        ValueNotFound(s.into())
    }
}

#[derive(Clone, Debug)]
pub struct HexField<T: AsRef<[u8]> + Clone>(pub T);

impl<T> Deref for HexField<T>
    where T: AsRef<[u8]> + Clone
{
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T> Serialize for HexField<T>
    where T: AsRef<[u8]> + Clone
{
    fn serialize<S>(&self, ser: &mut S) -> Result<(), S::Error>
        where S: Serializer
    {
        ser.serialize_str(&self.0.as_ref().to_hex())
    }
}

struct HexVisitor<T>
    where T: AsRef<[u8]> + HexValue
{
    _p: PhantomData<T>,
}

impl<T> Visitor for HexVisitor<T>
    where T: AsRef<[u8]> + HexValue + Clone
{
    type Value = HexField<T>;

    fn visit_str<E>(&mut self, s: &str) -> Result<HexField<T>, E>
        where E: de::Error
    {
        let v = T::from_hex(s).map_err(|_| de::Error::custom("Invalid hex"))?;
        Ok(HexField(v))
    }
}

impl<T> Deserialize for HexField<T>
    where T: AsRef<[u8]> + HexValue + Clone
{
    fn deserialize<D>(deserializer: &mut D) -> Result<Self, D::Error>
        where D: Deserializer
    {
        deserializer.deserialize_str(HexVisitor { _p: PhantomData })
    }
}

#[derive(Serialize)]
struct VoteValue {
    revoke: bool,
}

#[derive(Serialize)]
struct ConfigProposeInfo {
    from: PublicKey,
    height: u64,
    config: Vec<u8>,
    actual_from_height: u64,
}

#[derive(Deserialize)]
struct ConfigProposeRequest {
    pub config: Vec<u8>,
    pub actual_from_height: u64,
}

#[derive(Deserialize)]
struct ConfigVoteRequest {
    pub height: u64,
    pub hash_propose: Hash,
    pub seed: u64,
    pub revoke: bool,
}

impl ConfigProposeRequest {
    fn into_tx(&self, height: u64, pub_key: &PublicKey, sec_key: &SecretKey) -> TxConfigPropose {
        TxConfigPropose::new(pub_key,
                             height,
                             self.config.as_ref(),
                             self.actual_from_height,
                             &sec_key)
    }
}

impl ConfigVoteRequest {
    fn into_tx(&self, height: u64, pub_key: &PublicKey, sec_key: &SecretKey) -> TxConfigVote {
        TxConfigVote::new(pub_key,
                          height,
                          &self.hash_propose,
                          self.seed,
                          self.revoke,
                          &sec_key)
    }
}

impl From<TxConfigPropose> for ConfigProposeInfo {
    fn from(src: TxConfigPropose) -> ConfigProposeInfo {
        ConfigProposeInfo {
            from: *src.from(),
            height: src.height(),
            config: src.config().to_vec(),
            actual_from_height: src.actual_from_height(),
        }
    }
}

pub fn make_api<T>(api: &mut Api, b: Blockchain, channel: TxSender<NodeChannel>, cfg: NodeConfig)
    where T: TransactionInfo + From<RawTransaction>
{

    // let node = Node::new(b.clone(), cfg.clone());
    // let channel = node.channel();

    api.namespace("blockchain", move |api| {
        api.get("config/actual", |endpoint| {
            let b = b.clone();
            endpoint.summary("Returns actual configuration");
            endpoint.handle(move |client, _| {
                match Schema::new(&b.view()).get_actual_configuration() {
                    Ok(config) => client.json(&config.to_json()),
                    Err(e) => client.error(e),
                }
            })
        });
        api.put("config/propose", |endpoint| {
            let b = b.clone();
            let c = cfg.clone();
            let ch = channel.clone();
            endpoint.summary("Puts new ConfigPropose");            
            endpoint.params(|params| {
                params.opt_typed("config", json_dsl::array_of(json_dsl::u64()));                
                params.opt_typed("actual_from_height", json_dsl::u64());
            });
            endpoint.handle(move |client, params| {
                match from_value::<ConfigProposeRequest>(params.clone()) {
                    Ok(config_propose_request) => {
                        let config_propose =
                            config_propose_request.into_tx(Schema::new(&b.view()).last_height().unwrap(), &c.public_key, &c.secret_key);

                        let tx = ConfigTx::ConfigPropose(config_propose);
                        let tx_hash = HexValue::to_hex(&tx.hash());
                        match ch.send(tx) {
                            Ok(_) => {
                                let json = &jsonway::object(|json| json.set("tx_hash", tx_hash))
                                    .unwrap();
                                client.json(json)
                            }
                            Err(e) => {
                                client.error(e)
                            }
                        }
                    }
                    Err(_) => client.error(ValueNotFound::new("Can't parse ConfigPropose request")),
                }
            })
        });
        api.put("config/vote", |endpoint| {
            let b = b.clone();
            let c = cfg.clone();
            let ch = channel.clone();
            endpoint.summary("Puts new ConfigVote");
            endpoint.params(|params| {
                params.opt_typed("height", json_dsl::u64());
                params.opt_typed("hash_propose", json_dsl::string());
                params.opt_typed("seed", json_dsl::u64());
                params.opt_typed("revoke", json_dsl::boolean());
            });

            endpoint.handle(move |client, params| {
                match from_value::<ConfigVoteRequest>(params.clone()) {
                    Ok(config_vote_request) => {
                        let config_vote = config_vote_request.into_tx(Schema::new(&b.view()).last_height().unwrap(), &c.public_key, &c.secret_key);

                        let tx = ConfigTx::ConfigVote(config_vote);
                        let tx_hash = HexValue::to_hex(&tx.hash());
                        match ch.send(tx) {
                            Ok(_) => {
                                let json = &jsonway::object(|json| json.set("tx_hash", tx_hash))
                                    .unwrap();
                                client.json(json)
                            }
                            Err(e) => client.error(e),
                        }
                    }
                    Err(_) => client.error(ValueNotFound::new("Can't parse ConfigVote request")),
                }
            })
        });
        api.get("config/propose/:hash", |endpoint| {
            let b = b.clone();            
            endpoint.summary("Returns config propose by hash");
            endpoint.params(|params| {
                params.opt_typed("hash", json_dsl::string());
            });
            endpoint.handle(move |client, params| {
                let hash_propose = Hash::from_hex(params.find("hash").unwrap().as_str().unwrap())
                    .unwrap();                                    
                match ConfigurationSchema::new(&b.view()).get_config_propose(&hash_propose).unwrap() {
                    Some(config_propose) => {
                        let info = ConfigProposeInfo::from(config_propose);
                        return client.json(&info.to_json());
                    }
                    None => {
                        return client.error(ValueNotFound::new("Unable to find ConfigPropose for hash"))
                    }
                }                          
            })
        });
        api.get("config/vote/:from", |endpoint| {
            let b = b.clone(); 
            endpoint.summary("Returns config votes array");
            endpoint.params(|params| {
                params.opt_typed("from", json_dsl::string());
            });
            endpoint.handle(move |client, params| {
                let from = PublicKey::from_hex(params.find("from").unwrap().as_str().unwrap())
                    .unwrap();                
                match ConfigurationSchema::new(&b.view()).get_vote(&from).unwrap() {
                    Some(vote) => {
                        let vote_value = VoteValue { revoke: vote.revoke() };
                        client.json(&vote_value.to_json())
                    }
                    None => {
                        return client.error(ValueNotFound::new("Unable to find vote for public key"));
                    }
                }                
            })
        });
        api.get("blocks", |endpoint| {
            let b = b.clone();
            let c = cfg.clone();

            endpoint.summary("Returns blockchain info array");
            endpoint.params(|params| {
                params.opt_typed("from", json_dsl::u64());
                params.opt_typed("count", json_dsl::u64())
            });

            endpoint.handle(move |client, params| {
                let from = params.find("from").map(|x| x.as_u64().unwrap()).map(|x| x + 1);
                let count = params.find("count").map(|x| x.as_u64().unwrap()).unwrap_or(100);

                let view = b.clone().view();
                let explorer = BlockchainExplorer::new(&view, c.clone());
                match explorer.blocks_range::<T>(count, from) {
                    Ok(blocks) => client.json(&blocks.to_json()),
                    Err(e) => client.error(e),
                }
            })
        });
        api.get("blocks/:height", |endpoint| {
            let b = b.clone();
            let c = cfg.clone();

            endpoint.summary("Returns block with given height");
            endpoint.params(|params| {
                params.req_typed("height", json_dsl::u64());
            });

            endpoint.handle(move |client, params| {
                let height = params.find("height").unwrap().as_u64().unwrap();

                let view = b.clone().view();
                let explorer = BlockchainExplorer::new(&view, c.clone());
                match explorer.block_info_with_height::<T>(height) {
                    Ok(Some(block)) => client.json(&block.to_json()),
                    Ok(None) => {
                        client.error(ValueNotFound::new("Unable to find block with given height"))
                    }
                    Err(e) => client.error(e),
                }
            })
        });
        api.get("transactions/:hash", |endpoint| {
            let b = b.clone();
            let c = cfg.clone();

            endpoint.summary("Returns transaction info with given hash");
            endpoint.params(|params| {
                params.req_typed("hash", json_dsl::string());
            });

            endpoint.handle(move |client, params| {
                let hash = params.find("hash").unwrap().as_str().unwrap();
                let view = b.clone().view();
                let explorer = BlockchainExplorer::new(&view, c.clone());
                match Hash::from_hex(hash) {
                    Ok(hash) => {
                        match explorer.tx_info::<T>(&hash) {
                            Ok(Some(tx_info)) => client.json(&tx_info.to_json()),
                            Ok(None) => {
                                client.error(ValueNotFound::new("Unable to found transaction \
                                                                 with given hash"))
                            }
                            Err(e) => client.error(e),
                        }
                    }
                    Err(_) => client.error(StorageError::new("Unable to decode transaction hash")),
                }
            })
        });
    }) // namespace blockchain
}

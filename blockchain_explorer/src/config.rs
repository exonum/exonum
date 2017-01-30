#![feature(type_ascription)]
#![feature(proc_macro)]

use std::ops::Deref;
use std::marker::PhantomData;
use std::error;
use std::fmt;
use jsonway;
use serde_json::value::from_value;

use serde::{Serialize, Serializer};
use serde::de;
use serde::de::{Visitor, Deserialize, Deserializer};
use rustless::json::ToJson;
use rustless::{Api, Nesting};
use valico::json_dsl;

use exonum::crypto::{Hash, PublicKey, SecretKey, HexValue, ToHex};
use exonum::storage::Error as StorageError;
use exonum::blockchain::Blockchain;
use exonum::storage::Map;
use exonum::blockchain::Schema;
use exonum::node::NodeConfig;
use exonum::services::configuration::{TxConfigPropose, TxConfigVote, ConfigTx, ConfigurationSchema};
use ::ValueNotFound;
use exonum::messages::RawTransaction;
use exonum::node::NodeChannel;
use exonum::node::TxSender;
pub use explorer::{TransactionInfo, BlockchainExplorer, BlockInfo};

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
    pub height: u64,
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
    fn into_tx(&self, pub_key: &PublicKey, sec_key: &SecretKey) -> TxConfigPropose {
        TxConfigPropose::new(pub_key,
                             self.height,
                             self.config.as_ref(),
                             self.actual_from_height,
                             &sec_key)
    }
}

impl ConfigVoteRequest {
    fn into_tx(&self, pub_key: &PublicKey, sec_key: &SecretKey) -> TxConfigVote {
        TxConfigVote::new(pub_key,
                          self.height,
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

pub fn make_api<T>(api: &mut Api, b: Blockchain, tx_sender: TxSender<NodeChannel>, cfg: NodeConfig)
    where T: TransactionInfo + From<RawTransaction>
{
    api.namespace("configs", move |api| {
        api.get("actual", |endpoint| {
            let b = b.clone();
            endpoint.summary("Returns actual configuration");
            endpoint.handle(move |client, _| {
                match Schema::new(&b.view()).get_actual_configuration() {
                    Ok(config) => client.json(&config.to_json()),
                    Err(e) => client.error(e),
                }
            })
        });
        api.put("propose", |endpoint| {
            let b = b.clone();
            let c = cfg.clone();
            endpoint.summary("Puts new ConfigPropose");
            endpoint.params(|params| {
                params.opt_typed("config", json_dsl::string());
                params.opt_typed("height", json_dsl::u64());
                params.opt_typed("actual_from_height", json_dsl::u64());
            });
            endpoint.handle(move |client, params| {
                match from_value::<ConfigProposeRequest>(params.clone()) {
                    Ok(config_propose_request) => {
                        let config_propose =
                            config_propose_request.into_tx(&c.public_key, &c.secret_key);
                        match tx_sender.send(ConfigTx::ConfigPropose(config_propose)) {
                            Ok(tx_hash) => {
                                let json = &jsonway::object(|json| json.set("tx_hash", tx_hash))
                                    .unwrap();
                                client.json(json)
                            }
                            Err(e) => client.error(e),
                        }
                    }
                    Err(_) => client.error(ValueNotFound::new("Can't parse ConfigPropose request")),
                }
            })
        });
        api.put("vote", |endpoint| {
            let b = b.clone();
            let c = cfg.clone();
            endpoint.summary("Puts new ConfigVote");
            endpoint.params(|params| {
                params.opt_typed("height", json_dsl::u64());
                params.opt_typed("hash_propose", json_dsl::string());
                params.opt_typed("seed", json_dsl::u64());
                params.opt_typed("revoke", json_dsl::boolean());
            });

            endpoint.handle(move |client, params| {
                let sender = tx_sender.clone();
                match from_value::<ConfigVoteRequest>(params.clone()) {
                    Ok(config_vote_request) => {
                        let config_vote = config_vote_request.into_tx(&c.public_key, &c.secret_key);
                        match sender.send(ConfigTx::ConfigVote(config_vote)) {
                            Ok(tx_hash) => {
                                // let json = &jsonway::object(|json| json.set("tx_hash", tx_hash))
                                //     .unwrap();
                                // client.json(json)
                                client.error(ValueNotFound::new("Can't parse ConfigVote request"))
                            }
                            Err(e) => client.error(e),
                        }
                    }
                    Err(_) => client.error(ValueNotFound::new("Can't parse ConfigVote request")),
                }
            })
        });
        api.get("propose/:hash", |endpoint| {
            let b = b.clone();
            endpoint.summary("Returns config propose by hash");
            endpoint.params(|params| {
                params.opt_typed("hash", json_dsl::string());
            });
            endpoint.handle(move |client, params| {
                let hash_propose = Hash::from_hex(params.find("hash").unwrap().as_str().unwrap())
                    .unwrap();
                match ConfigurationSchema::new(&b.view())
                    .get_config_propose(&hash_propose)
                    .unwrap() {
                    Some(config_propose) => {
                        let info = ConfigProposeInfo::from(config_propose);
                        client.json(&info.to_json())
                    }
                    None => {
                        client.error(ValueNotFound::new("Unable to find ConfigPropose for hash"))
                    }
                }
            })
        });
        api.get("vote/:from", |endpoint| {
            let b = b.clone();
            endpoint.summary("Returns config votes array");
            endpoint.params(|params| {
                params.opt_typed("from", json_dsl::string());
            });
            endpoint.handle(move |client, params| {
                let from = PublicKey::from_hex(params.find("from").unwrap().as_str().unwrap())
                    .unwrap();
                let view = b.view();
                match ConfigurationSchema::new(&b.view()).get_vote(&from).unwrap() {
                    Some(vote) => {
                        let vote_value = VoteValue { revoke: vote.revoke() };
                        client.json(&vote_value.to_json())
                    }
                    None => client.error(ValueNotFound::new("Unable to find vote for public key")),
                }
            })
        });

    }) // namespace configs
}

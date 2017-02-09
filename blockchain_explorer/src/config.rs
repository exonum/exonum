use jsonway;
use serde_json::value::from_value;

use rustless::json::ToJson;
use rustless::{Api, Nesting};
use valico::json_dsl;

use exonum::crypto::{Hash, PublicKey, SecretKey, HexValue};
use exonum::blockchain::{Blockchain, Schema};
use exonum::node::NodeConfig;
use exonum::services::configuration::{TxConfigPropose, TxConfigVote, ConfigTx, ConfigurationSchema};
use ::ValueNotFound;
use exonum::messages::RawTransaction;
use exonum::node::NodeChannel;
use exonum::node::TxSender;
use exonum::blockchain::config::{ConsensusConfig, StoredConfiguration};
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
    pub actual_from: u64,
    pub validators: Vec<PublicKey>,
    pub consensus: ConsensusConfig,
}

#[derive(Deserialize)]
struct ConfigVoteRequest {
    pub hash_propose: Hash,
    pub seed: u64,
    pub revoke: bool,
}

#[derive(Clone)]
pub struct ConfigApi {
    pub blockchain: Blockchain,
    pub channel: TxSender,
    pub cfg: NodeConfig,
}

impl ConfigApi {}

impl Api for ConfigApi {
    fn wire(&self, router: &mut Router) {}
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
            let c = cfg.clone();
            let b = b.clone();
            let sender = tx_sender.clone();
            endpoint.summary("Puts new ConfigPropose");
            endpoint.params(|params| {
                params.req_typed("actual_from", json_dsl::u64());
                params.req_types("validators", json_dsl::array_of(json_dsl::string()));
                params.req_nested("consensus", |params| {
                    params.req_typed("round_timeout", json_dsl::u64);
                    params.req_typed("status_timeout", json_dsl::u64);
                    params.req_typed("peers_timeout", json_dsl::u64);
                    params.req_typed("propose_timeout", json_dsl::u64);
                    params.req_typed("txs_block_limit", json_dsl::u64);
                });
            });
            endpoint.handle(move |client, params| {
                match from_value::<ConfigProposeRequest>(params.clone()) {
                    Ok(config_propose_request) => {
                        let height = Schema::new(&b.view()).last_height().unwrap();
                        let config = StoredConfiguration {
                            actual_from: params.find("actual_from").unwrap(),
                            validators: params.find("validators").unwrap(),
                            consensus: from_value::<ConsensusConfig>(params.find("consensus")
                                .unwrap()
                                .clone()),
                        };
                        let config_propose = TxConfigPropose::new(&c.public_key,
                                                                  height,
                                                                  config.serialize(),
                                                                  &c.secret_key);
                        match sender.send(ConfigTx::ConfigPropose(config_propose)) {
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
            let c = cfg.clone();
            let b = b.clone();
            let sender = tx_sender.clone();
            endpoint.summary("Puts new ConfigVote");
            endpoint.params(|params| {
                params.opt_typed("hash_propose", json_dsl::string());
                params.opt_typed("seed", json_dsl::u64());
                params.opt_typed("revoke", json_dsl::boolean());
            });

            endpoint.handle(move |client, params| {
                match from_value::<ConfigVoteRequest>(params.clone()) {
                    Ok(config_vote_request) => {
                        let height = Schema::new(&b.view()).last_height().unwrap();
                        let config_vote =
                            config_vote_request.into_tx(height, &c.public_key, &c.secret_key);
                        match sender.send(ConfigTx::ConfigVote(config_vote)) {
                            Ok(tx_hash) => {
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

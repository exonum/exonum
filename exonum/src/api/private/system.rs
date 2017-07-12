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

use serde_json::Value;

use std::net::SocketAddr;

use router::Router;
use iron::prelude::*;

use params::{Params, Value as ParamsValue};
use node::{NodeChannel, ApiSender};
use node::state::TxPool;
use blockchain::{Service, Blockchain, SharedNodeState};
use crypto::{Hash, HexValue};
use explorer::{TxInfo, BlockchainExplorer};
use api::{Api, ApiError};
use messages::{TEST_NETWORK_ID, PROTOCOL_MAJOR_VERSION};

#[derive(Serialize, Clone, Debug)]
struct ServiceInfo {
    name: String,
    id: u16,
}

/// `DTO` is used to transfer information about node.
#[derive(Serialize, Clone, Debug)]
pub struct NodeInfo {
    network_id: u8,
    protocol_version: u8,
    services: Vec<ServiceInfo>,
}

impl NodeInfo {
    /// Creates new `NodeInfo`, from services list.
    pub fn new<'a, I>(services: I) -> NodeInfo
    where
        I: IntoIterator<Item = &'a Box<Service>>,
    {
        NodeInfo {
            network_id: TEST_NETWORK_ID,
            protocol_version: PROTOCOL_MAJOR_VERSION,
            services: services
                .into_iter()
                .map(|s| {
                    ServiceInfo {
                        name: s.service_name().to_owned(),
                        id: s.service_id(),
                    }
                })
                .collect(),
        }
    }
}


#[derive(Serialize, Debug)]
struct PeerInfo {
    addr: SocketAddr,
    delay: u64,
}

#[derive(Serialize)]
struct PeersInfo {
    incoming_connections: Vec<SocketAddr>,
    outgoing_connections: Vec<SocketAddr>,
    reconnects: Vec<PeerInfo>,
}

#[derive(Serialize)]
struct MemPoolTxInfo {
    content: Value,
}

#[derive(Serialize)]
#[serde(tag = "type")]
enum MemPoolResult {
    Unknown,
    MemPool(MemPoolTxInfo),
    Commited(TxInfo),
}

#[derive(Serialize)]
struct MemPoolInfo {
    size: usize,
}

#[derive(Clone, Debug)]
pub struct SystemApi {
    blockchain: Blockchain,
    pool: TxPool,
    info: NodeInfo,
    shared_api_state: SharedNodeState,
    node_channel: ApiSender<NodeChannel>,
}

impl SystemApi {
    /// Creates new `SystemApi`, from `ApiContext`
    pub fn new(
        info: NodeInfo,
        blockchain: Blockchain,
        pool: TxPool,
        shared_api_state: SharedNodeState,
        node_channel: ApiSender<NodeChannel>,
    ) -> SystemApi {
        SystemApi {
            info,
            blockchain,
            node_channel,
            pool,
            shared_api_state,
        }
    }

    fn get_mempool_info(&self) -> MemPoolInfo {
        MemPoolInfo { size: self.pool.read().expect("Expected read lock").len() }
    }

    fn get_peers_info(&self) -> PeersInfo {
        PeersInfo {
            incoming_connections: self.shared_api_state.incoming_connections(),
            outgoing_connections: self.shared_api_state.outgoing_connections(),
            reconnects: self.shared_api_state
                .reconnects_timeout()
                .into_iter()
                .map(|(s, d)| PeerInfo { addr: s, delay: d })
                .collect(),
        }
    }

    fn get_network_info(&self) -> NodeInfo {
        self.info.clone()
    }

    fn get_mempool_tx(&self, hash_str: &str) -> Result<MemPoolResult, ApiError> {
        let hash = Hash::from_hex(hash_str)?;
        self.pool
            .read()
            .expect("Expected read lock")
            .get(&hash)
            .map_or_else(
                || {
                    let explorer = BlockchainExplorer::new(&self.blockchain);
                    Ok(explorer.tx_info(&hash)?.map_or(
                        MemPoolResult::Unknown,
                        MemPoolResult::Commited,
                    ))
                },
                |o| Ok(MemPoolResult::MemPool(MemPoolTxInfo { content: o.info() })),
            )

    }

    fn peer_add(&self, ip_str: &str) -> Result<(), ApiError> {
        let addr: SocketAddr = ip_str.parse()?;
        self.node_channel.peer_add(addr)?;
        Ok(())
    }
}

impl Api for SystemApi {
    fn wire(&self, router: &mut Router) {
        let _self = self.clone();
        let mempool = move |req: &mut Request| -> IronResult<Response> {
            let params = req.extensions.get::<Router>().unwrap();
            match params.find("hash") {
                Some(hash_str) => {
                    let info = _self.get_mempool_tx(hash_str)?;
                    _self.ok_response(&::serde_json::to_value(info).unwrap())
                }
                None => {
                    Err(ApiError::IncorrectRequest(
                        "Required parameter of transaction 'hash' is missing".into(),
                    ))?
                }
            }
        };

        let _self = self.clone();
        let mempool_info = move |_: &mut Request| -> IronResult<Response> {
            let info = _self.get_mempool_info();
            _self.ok_response(&::serde_json::to_value(info).unwrap())
        };

        let _self = self.clone();
        let peer_add = move |req: &mut Request| -> IronResult<Response> {
            let map = req.get_ref::<Params>().unwrap();
            match map.find(&["ip"]) {
                Some(&ParamsValue::String(ref ip_str)) => {
                    _self.peer_add(ip_str)?;
                    _self.ok_response(&::serde_json::to_value("Ok").unwrap())
                }
                _ => {
                    Err(ApiError::IncorrectRequest(
                        "Required parameter of peer 'ip' is missing".into(),
                    ))?
                }
            }
        };

        let _self = self.clone();
        let peers_info = move |_: &mut Request| -> IronResult<Response> {
            let info = _self.get_peers_info();
            _self.ok_response(&::serde_json::to_value(info).unwrap())
        };

        let _self = self.clone();
        let network = move |_: &mut Request| -> IronResult<Response> {
            let info = _self.get_network_info();
            _self.ok_response(&::serde_json::to_value(info).unwrap())
        };

        router.get("/v1/mempool", mempool_info, "mempool");
        router.get("/v1/mempool/:hash", mempool, "mempool_tx");
        router.get("/v1/peers", peers_info, "peers_info");
        router.post("/v1/peeradd", peer_add, "peer_add");
        router.get("/v1/network", network, "network_info");
    }
}

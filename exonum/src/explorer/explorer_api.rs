use std::num::ParseIntError;
use std::str::ParseBoolError;
use std::net::SocketAddr;

use params::{Params, Value};
use router::Router;
use iron::prelude::*;

use node::{NodeChannel, ApiSender };
use node::state::TxPool;
use blockchain::{Blockchain, Block, SharedNodeState};
use crypto::{Hash, HexValue};
use explorer::{TxInfo, BlockInfo, BlockchainExplorer};
use api::{Api, ApiError};

const MAX_BLOCKS_PER_REQUEST: u64 = 1000;

#[derive(Serialize)]
struct PeersInfo {
    incoming_connections: Vec<SocketAddr>,
    outgoing_connections: Vec<SocketAddr>,
    reconnects: Vec<(SocketAddr, u64)>
}

#[derive(Serialize)]
enum MemPoolResult {
    Unknown,
    MemPool(::serde_json::Value),
    Commited(TxInfo)
}

#[derive(Serialize)]
struct MemPoolInfo {
    size: usize,
}

#[derive(Clone, Debug)]
pub struct ExplorerApi {
    pool: TxPool,
    shared_api_state: SharedNodeState,
    blockchain: Blockchain,
    node_channel: ApiSender<NodeChannel>,
}

impl ExplorerApi {
    pub fn new(
        blockchain: Blockchain,
        pool: TxPool,
        shared_api_state: SharedNodeState,
        node_channel: ApiSender<NodeChannel>
    ) -> ExplorerApi {
        ExplorerApi {
            blockchain, pool, shared_api_state, node_channel
        }
    }

    fn get_blocks(&self, count: u64, from: Option<u64>, skip_empty_blocks: bool) -> Result<Vec<Block>, ApiError> {
        if count > MAX_BLOCKS_PER_REQUEST {
             return Err(ApiError::IncorrectRequest(
                 format!("Max block count per request exceeded ({})", MAX_BLOCKS_PER_REQUEST).into()))
        }
        let explorer = BlockchainExplorer::new(&self.blockchain);
        Ok(explorer.blocks_range(count, from, skip_empty_blocks))
    }

    fn get_block(&self, height: u64) -> Result<Option<BlockInfo>, ApiError> {
        let explorer = BlockchainExplorer::new(&self.blockchain);
        Ok(explorer.block_info(height))
    }

    fn get_transaction(&self, hash_str: &str) -> Result<Option<TxInfo>, ApiError> {
        let explorer = BlockchainExplorer::new(&self.blockchain);
        let hash = Hash::from_hex(hash_str)?;
        explorer.tx_info(&hash)
    }

    fn get_mempool_info(&self) -> MemPoolInfo {
        MemPoolInfo {
            size: self.pool.read().expect("Expected read lock").len()
        }
    }

    fn get_peers_info(&self) -> PeersInfo {
        PeersInfo{
            incoming_connections: self.shared_api_state
                                      .in_connections(),
            outgoing_connections: self.shared_api_state
                                      .out_connections(),
            reconnects: self.shared_api_state
                                      .reconnects_timeout(),
        }
    }

    fn get_mempool_tx(&self, hash_str: &str) -> Result<MemPoolResult, ApiError> {
        let hash = Hash::from_hex(hash_str)?;
        
        self.pool.read().expect("Expected read lock")
                        .get(&hash)
                        .map_or_else(
                            ||{
                                let explorer = BlockchainExplorer::new(&self.blockchain);
                                Ok(explorer.tx_info(&hash)?
                                           .map_or(MemPoolResult::Unknown, |i| MemPoolResult::Commited(i)))
                            },
                            |o| Ok(MemPoolResult::MemPool(o.info())))
                        
    }

    fn peer_add(&self, ip_str: &str) -> Result<(), ApiError> {
        let addr: SocketAddr = ip_str.parse()?;
        self.node_channel.peer_add(addr)?;
        Ok(())
    }
}

impl Api for ExplorerApi {
    fn wire(&self, router: &mut Router) {

        let _self = self.clone();
        let blocks = move |req: &mut Request| -> IronResult<Response> {
            let map = req.get_ref::<Params>().unwrap();
            let count: u64 = match map.find(&["count"]) {
                Some(&Value::String(ref count_str)) => {
                    count_str.parse().map_err(|e: ParseIntError| ApiError::IncorrectRequest(Box::new(e)))?
                }
                _ => {
                    return Err(ApiError::IncorrectRequest("Required parameter of blocks 'count' is missing".into()))?;
                }
            };
            let from: Option<u64> = match map.find(&["from"]) {
                Some(&Value::String(ref from_str)) => {
                    Some(from_str.parse().map_err(|e: ParseIntError| ApiError::IncorrectRequest(Box::new(e)))?)
                }
                _ => None,
            };
            let skip_empty_blocks: bool = match map.find(&["skip_empty_blocks"]) {
                Some(&Value::String(ref skip_str)) => {
                    skip_str.parse().map_err(|e: ParseBoolError| ApiError::IncorrectRequest(Box::new(e)))?
                }
                _ => false,
            };
            let info = _self
                .get_blocks(count, from, skip_empty_blocks)?;
            _self.ok_response(&::serde_json::to_value(info).unwrap())
        };

        let _self = self.clone();
        let block = move |req: &mut Request| -> IronResult<Response> {
            let params = req.extensions.get::<Router>().unwrap();
            match params.find("height") {
                Some(height_str) => {
                    let height: u64 = height_str.parse().map_err(|e: ParseIntError| ApiError::IncorrectRequest(Box::new(e)))?;
                    let info = _self.get_block(height)?;
                    _self.ok_response(&::serde_json::to_value(info).unwrap())
                }
                None => Err(ApiError::IncorrectRequest("Required parameter of block 'height' is missing".into()))?,
            }
        };

        let _self = self.clone();
        let transaction = move |req: &mut Request| -> IronResult<Response> {
            let params = req.extensions.get::<Router>().unwrap();
            match params.find("hash") {
                Some(hash_str) => {
                    let info = _self.get_transaction(hash_str)?;
                    _self.ok_response(&::serde_json::to_value(info).unwrap())
                }
                None => Err(ApiError::IncorrectRequest("Required parameter of transaction 'hash' is missing".into()))?,
            }
        };

        let _self = self.clone();
        let mempool = move |req: &mut Request| -> IronResult<Response> {
            let params = req.extensions.get::<Router>().unwrap();
            match params.find("hash") {
                Some(hash_str) => {
                    let info = _self.get_mempool_tx(hash_str)?;
                    _self.ok_response(&::serde_json::to_value(info).unwrap())
                }
                None => Err(ApiError::IncorrectRequest("Required parameter of transaction 'hash' is missing".into()))?,
            }
        };

        let _self = self.clone();
        let mempool_info = move |_: &mut Request| -> IronResult<Response> {
            let info = _self.get_mempool_info();
            _self.ok_response(&::serde_json::to_value(info).unwrap())
        };

        let _self = self.clone();
        let peer_add = move |req: &mut Request| -> IronResult<Response> {
            let params = req.extensions.get::<Router>().unwrap();
            match params.find("ip") {
                Some(ip_str) => {
                    let info = _self.peer_add(ip_str)?;
                    _self.ok_response(&::serde_json::to_value(info).unwrap())
                }
                None => Err(ApiError::IncorrectRequest("Required parameter of transaction 'hash' is missing".into()))?,
            }
        };

        let _self = self.clone();
        let peers_info = move |_: &mut Request| -> IronResult<Response> {
            let info = _self.get_peers_info();
            _self.ok_response(&::serde_json::to_value(info).unwrap())
        };

        router.get("/v1/blocks", blocks, "blocks");
        router.get("/v1/blocks/:height", block, "height");
        router.get("/v1/mempool", mempool_info, "mempool");
        router.get("/v1/mempool/:hash", mempool, "mempool_tx");

        router.get("/v1/peers", peers_info, "peers_info");
        router.get("/v1/peers/:ip", peer_add, "peer_add");

        router.get("/v1/transactions/:hash", transaction, "hash");
    }
    
}

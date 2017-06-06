use serde_json::Value as JValue;
use params::{Params, Value};
use router::Router;
use iron::prelude::*;

use blockchain::Blockchain;
use crypto::{Hash, HexValue};
use explorer::{BlockInfo, BlockchainExplorer};
use api::{Api, ApiError};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BlocksRequest {
    pub from: Option<u64>,
    pub count: u64,
}

#[derive(Clone, Debug)]
pub struct ExplorerApi {
    blockchain: Blockchain,
}

impl ExplorerApi {
    pub fn new(blockchain: Blockchain) -> ExplorerApi {
        ExplorerApi {
            blockchain: blockchain
        }
    }

    fn get_blocks(&self, blocks_request: BlocksRequest) -> Result<Vec<BlockInfo>, ApiError> {
        let explorer = BlockchainExplorer::new(&self.blockchain);
        match explorer.blocks_range(blocks_request.count, blocks_request.from) {
            Ok(blocks) => Ok(blocks),
            Err(e) => Err(ApiError::Storage(e)),
        }
    }

    fn get_block(&self, height: u64) -> Result<Option<BlockInfo>, ApiError> {
        let explorer = BlockchainExplorer::new(&self.blockchain);
        match explorer.block_info_with_height(height) {
            Ok(block_info) => Ok(block_info),
            Err(e) => Err(ApiError::Storage(e)),
        }
    }

    fn get_transaction(&self, hash_str: &str) -> Result<Option<JValue>, ApiError> {
        let explorer = BlockchainExplorer::new(&self.blockchain);
        let hash = Hash::from_hex(hash_str)?;
        match explorer.tx_info(&hash) {
            Ok(tx_info) => Ok(tx_info),
            Err(e) => Err(ApiError::Storage(e)),
        }
    }
}

impl Api for ExplorerApi {
    fn wire(&self, router: &mut Router) {

        let _self = self.clone();
        let blocks = move |req: &mut Request| -> IronResult<Response> {
            let map = req.get_ref::<Params>().unwrap();
            let count: u64;
            let from: Option<u64>;
            count = match map.find(&["count"]) {
                Some(&Value::String(ref count_str)) => {
                    count_str.parse().map_err(|_| ApiError::IncorrectRequest)?
                }
                _ => {
                    return Err(ApiError::IncorrectRequest)?;
                }
            };
            from = match map.find(&["from"]) {
                Some(&Value::String(ref from_str)) => {
                    Some(from_str.parse().map_err(|_| ApiError::IncorrectRequest)?)
                }
                _ => None,
            };
            let info = _self
                .get_blocks(BlocksRequest {
                                count: count,
                                from: from,
                            })?;
            _self.ok_response(&::serde_json::to_value(info).unwrap())
        };

        let _self = self.clone();
        let block = move |req: &mut Request| -> IronResult<Response> {
            let params = req.extensions.get::<Router>().unwrap();
            match params.find("height") {
                Some(height_str) => {
                    let height: u64 = height_str.parse().map_err(|_| ApiError::IncorrectRequest)?;
                    let info = _self.get_block(height)?;
                    _self.ok_response(&::serde_json::to_value(info).unwrap())
                }
                None => Err(ApiError::IncorrectRequest)?,
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
                None => Err(ApiError::IncorrectRequest)?,
            }
        };

        router.get("/v1/blocks", blocks, "blocks");
        router.get("/v1/blocks/:height", block, "height");
        router.get("/v1/transactions/:hash", transaction, "hash");
    }
}

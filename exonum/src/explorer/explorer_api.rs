use params::{Params, Value};
use router::Router;
use iron::prelude::*;

use blockchain::{Blockchain, Block};
use crypto::{Hash, HexValue};
use explorer::{TxInfo, BlockInfo, BlockchainExplorer};
use api::{Api, ApiError};

const MAX_BLOCKS_PER_REQUEST: u64 = 1000;

#[derive(Clone)]
pub struct ExplorerApi {
    blockchain: Blockchain,
}

impl ExplorerApi {
    pub fn new(blockchain: Blockchain) -> ExplorerApi {
        ExplorerApi {
            blockchain: blockchain
        }
    }

    fn get_blocks(&self, count: u64, from: Option<u64>, skip_empty_blocks: bool) -> Result<Vec<Block>, ApiError> {
        if count > MAX_BLOCKS_PER_REQUEST {
             return Err(ApiError::IncorrectRequest)
        }
        let explorer = BlockchainExplorer::new(&self.blockchain);
        Ok(explorer.blocks_range(count, from, skip_empty_blocks)?)
    }

    fn get_block(&self, height: u64) -> Result<Option<BlockInfo>, ApiError> {
        let explorer = BlockchainExplorer::new(&self.blockchain);
        match explorer.block_info(height) {
            Ok(block_info) => Ok(block_info),
            Err(e) => Err(ApiError::Storage(e)),
        }
    }

    fn get_transaction(&self, hash_str: &str) -> Result<Option<TxInfo>, ApiError> {
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
            let count: u64 = match map.find(&["count"]) {
                Some(&Value::String(ref count_str)) => {
                    count_str.parse().map_err(|_| ApiError::IncorrectRequest)?
                }
                _ => {
                    return Err(ApiError::IncorrectRequest)?;
                }
            };
            let from: Option<u64> = match map.find(&["from"]) {
                Some(&Value::String(ref from_str)) => {
                    Some(from_str.parse().map_err(|_| ApiError::IncorrectRequest)?)
                }
                _ => None,
            };
            let skip_empty_blocks: bool = match map.find(&["skip_empty_blocks"]) {
                Some(&Value::String(ref skip_str)) => {
                    skip_str.parse().map_err(|_| ApiError::IncorrectRequest)?
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

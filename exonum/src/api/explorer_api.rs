use std::num::ParseIntError;
use std::str::ParseBoolError;

use params::{Params, Value};
use router::Router;
use iron::prelude::*;

use blockchain::{Blockchain, Block};
use crypto::{Hash, HexValue};
use explorer::{TxInfo, BlockInfo, BlockchainExplorer};
use api::{Api, ApiError};

const MAX_BLOCKS_PER_REQUEST: u64 = 1000;

#[derive(Clone, Debug)]
pub struct ExplorerApi {
    blockchain: Blockchain,
}

impl ExplorerApi {
    pub fn new(
        blockchain: Blockchain
    ) -> ExplorerApi {
        ExplorerApi {
            blockchain,
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

        router.get("/v1/blocks", blocks, "blocks");
        router.get("/v1/blocks/:height", block, "height");
        router.get("/v1/transactions/:hash", transaction, "hash");
    }
    
}

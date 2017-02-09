use exonum::node::NodeConfig;
use exonum::blockchain::Blockchain;
use serde_json::value::ToJson;
use serde_json::Value as JValue;
use params::{Params, Value};
use router::Router;
use api::{Api, ApiError};
use iron::prelude::*;
use bodyparser;
use explorer::{BlockInfo, BlockchainExplorer};
use exonum::crypto::{Hash, HexValue};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BlocksRequest {
    pub from: Option<u64>,
    pub count: u64,
}

#[derive(Clone)]
pub struct ExplorerApi {
    pub blockchain: Blockchain,
    pub cfg: NodeConfig,
}

impl ExplorerApi {
    fn get_blocks(&self, blocks_request: BlocksRequest) -> Result<Vec<BlockInfo>, ApiError> {
        let explorer = BlockchainExplorer::new(&self.blockchain, self.cfg.clone());
        match explorer.blocks_range(blocks_request.count, blocks_request.from) {
            Ok(blocks) => Ok(blocks),
            Err(e) => Err(ApiError::Storage(e)),
        }
    }

    fn get_block(&self, height: u64) -> Result<Option<BlockInfo>, ApiError> {
        let explorer = BlockchainExplorer::new(&self.blockchain, self.cfg.clone());
        match explorer.block_info_with_height(height) {
            Ok(block_info) => Ok(block_info),
            Err(e) => Err(ApiError::Storage(e)),
        }
    }

    fn get_transaction(&self, hash_str: &String) -> Result<Option<JValue>, ApiError> {
        let explorer = BlockchainExplorer::new(&self.blockchain, self.cfg.clone());
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
            match req.get::<bodyparser::Struct<BlocksRequest>>().unwrap() {
                Some(request) => {
                    let info = _self.get_blocks(request)?;
                    _self.ok_response(&info.to_json())
                }
                None => Err(ApiError::IncorrectRequest)?,
            }
        };

        let _self = self.clone();
        let block = move |req: &mut Request| -> IronResult<Response> {
            let map = req.get_ref::<Params>().unwrap();
            match map.find(&["block"]) {
                Some(&Value::U64(height)) => {
                    let info = _self.get_block(height)?;
                    _self.ok_response(&info.to_json())
                }
                _ => return Err(ApiError::IncorrectRequest)?,
            }
        };

        let _self = self.clone();
        let transaction = move |req: &mut Request| -> IronResult<Response> {
            let map = req.get_ref::<Params>().unwrap();
            match map.find(&["hash"]) {
                Some(&Value::String(ref hash)) => {
                    let info = _self.get_transaction(hash)?;
                    _self.ok_response(&info.to_json())
                }
                _ => return Err(ApiError::IncorrectRequest)?,
            }
        };

        router.get("/v1/api/blockchain/blocks", blocks, "blocks");
        router.get("/v1/api/blockchain/blocks/:height", block, "height");
        router.get("/v1/api/blockchain/transactions/:hash", transaction, "hash");

    }
}
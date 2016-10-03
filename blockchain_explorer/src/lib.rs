#![feature(type_ascription)]
#![feature(custom_derive)]
#![feature(plugin)]
#![plugin(serde_macros)]
#![feature(question_mark)]

extern crate time;
extern crate serde;
extern crate exonum;
extern crate rustless;
extern crate valico;

use std::cmp;

use serde::{Serialize, Serializer};
use rustless::json::ToJson;
use rustless::{Api, Nesting};
use valico::json_dsl;

use exonum::crypto::{Hash, HexValue};
use exonum::storage::{Map, List};
use exonum::storage::{Error as StorageError, Result as StorageResult};
use exonum::blockchain::{Blockchain, View};

pub struct BlockchainExplorer<B: Blockchain> {
    view: B::View,
}

pub trait TransactionInfo: Serialize {}

#[derive(Debug)]
pub struct BlockInfo<T>
    where T: TransactionInfo
{
    height: u64,
    // proposer: PublicKey, // TODO add to block dto
    propose_time: i64,

    prev_hash: Hash,
    hash: Hash,
    state_hash: Hash,
    tx_hash: Hash,
    txs: Vec<T>,
}

impl<T> Serialize for BlockInfo<T>
    where T: TransactionInfo
{
    fn serialize<S>(&self, ser: &mut S) -> Result<(), S::Error>
        where S: Serializer
    {
        let mut state = ser.serialize_struct("block_info", 8)?;
        ser.serialize_struct_elt(&mut state, "height", self.height)?;
        //ser.serialize_struct_elt(&mut state, "propose_time", self.proposer.to_hex())?;
        ser.serialize_struct_elt(&mut state, "propose_time", self.propose_time)?;

        ser.serialize_struct_elt(&mut state, "prev_hash", self.prev_hash.to_hex())?;
        ser.serialize_struct_elt(&mut state, "hash", self.hash.to_hex())?;
        ser.serialize_struct_elt(&mut state, "state_hash", self.state_hash.to_hex())?;
        ser.serialize_struct_elt(&mut state, "tx_hash", self.tx_hash.to_hex())?;
        ser.serialize_struct_elt(&mut state, "txs", &self.txs)?;
        ser.serialize_struct_end(state)
    }
}


impl<B: Blockchain> BlockchainExplorer<B> {
    pub fn new(b: B) -> BlockchainExplorer<B> {
        BlockchainExplorer { view: b.view() }
    }

    pub fn from_view(view: B::View) -> BlockchainExplorer<B> {
        BlockchainExplorer { view: view }
    }

    pub fn tx_info<T>(&self, tx_hash: &Hash) -> StorageResult<Option<T>>
        where T: TransactionInfo + From<B::Transaction>
    {
        let tx = self.view.transactions().get(tx_hash)?;
        Ok(tx.map(|tx| T::from(tx)))
    }

    pub fn block_info<T>(&self, block_hash: &Hash) -> StorageResult<Option<BlockInfo<T>>>
        where T: TransactionInfo + From<B::Transaction>
    {
        let block = self.view.blocks().get(block_hash)?;
        if let Some(block) = block {
            let block_txs = self.block_txs(block.height())?;
            let info = BlockInfo {
                height: block.height(),
                // proposer: block.proposer(),
                propose_time: block.time().sec,

                prev_hash: *block.prev_hash(),
                hash: *block_hash,
                state_hash: *block.state_hash(),
                tx_hash: *block.tx_hash(),
                txs: block_txs,
            };
            Ok(Some(info))
        } else {
            Ok(None)
        }
    }

    pub fn block_info_with_height<T>(&self, height: u64) -> StorageResult<Option<BlockInfo<T>>>
        where T: TransactionInfo + From<B::Transaction>
    {
        if let Some(block_hash) = self.view.heights().get(height)? {
            // TODO avoid double unwrap
            self.block_info(&block_hash)
        } else {
            Ok(None)
        }
    }

    pub fn blocks_range<T>(&self, from: u64, to: Option<u64>) -> StorageResult<Vec<BlockInfo<T>>>
        where T: TransactionInfo + From<B::Transaction>
    {
        let heights = self.view.heights();

        let max_len = heights.len()?;
        let len = cmp::min(max_len, to.unwrap_or(max_len));

        let mut v = Vec::new();
        for height in from..len {
            if let Some(ref h) = heights.get(height)? {
                if let Some(block_info) = self.block_info(h)? {
                    v.push(block_info);
                }
            }
        }
        Ok(v)
    }

    fn block_txs<T>(&self, height: u64) -> StorageResult<Vec<T>>
        where T: TransactionInfo + From<B::Transaction>
    {
        let txs = self.view.block_txs(height);
        let tx_count = txs.len()?;

        let mut v = Vec::new();
        for i in 0..tx_count {
            if let Some(tx_hash) = txs.get(i)? {
                if let Some(tx_info) = self.tx_info(&tx_hash)? {
                    v.push(tx_info);
                }
            }
        }
        Ok(v)
    }
}

pub fn make_api<B, T>(api: &mut Api, b1: B)
    where B: Blockchain,
          T: TransactionInfo + From<B::Transaction>
{
    api.namespace("blockchain", move |api| {
        api.get("block", |endpoint| {
            let b1 = b1.clone();

            endpoint.summary("Returns blockchain info array");
            endpoint.params(|params| {
                params.opt_typed("from", json_dsl::u64());
                params.opt_typed("to", json_dsl::u64())
            });

            endpoint.handle(move |client, params| {
                let from = params.find("from").map(|x| x.as_u64().unwrap()).unwrap_or(0);
                let to = params.find("to").map(|x| x.as_u64().unwrap());

                let explorer = BlockchainExplorer::new(b1.clone());
                match explorer.blocks_range::<T>(from, to) {
                    Ok(blocks) => client.json(&blocks.to_json()),
                    Err(e) => client.error(e),
                }
            })
        });
        api.get("block/:height", |endpoint| {
            let b1 = b1.clone();

            endpoint.summary("Returns block with given height");
            endpoint.params(|params| {
                params.req_typed("height", json_dsl::u64());
            });

            endpoint.handle(move |client, params| {
                let height = params.find("height").unwrap().as_u64().unwrap();

                let explorer = BlockchainExplorer::new(b1.clone());
                match explorer.block_info_with_height::<T>(height) {
                    Ok(Some(block)) => client.json(&block.to_json()),
                    Ok(None) => Ok(client),
                    Err(e) => client.error(e),
                }
            })
        });
        api.get("transaction/:hash", |endpoint| {
            let b1 = b1.clone();

            endpoint.summary("Returns transaction info with given hash");
            endpoint.params(|params| {
                params.req_typed("hash", json_dsl::string());
            });

            endpoint.handle(move |client, params| {
                let hash = params.find("hash").unwrap().as_str().unwrap();
                let explorer = BlockchainExplorer::new(b1.clone());
                match Hash::from_hex(hash) {
                    Ok(hash) => {
                        match explorer.tx_info::<T>(&hash) {
                            Ok(tx_info) => client.json(&tx_info.to_json()),
                            Err(e) => client.error(e),
                        }
                    }
                    Err(_) => client.error(StorageError::new("Unable to decode transaction hash")),
                }
            })
        });
    }) // namespace blockchain
}

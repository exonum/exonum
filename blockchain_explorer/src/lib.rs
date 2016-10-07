#![feature(type_ascription)]
#![feature(custom_derive)]
#![feature(plugin)]
#![plugin(serde_macros)]
#![feature(question_mark)]

mod explorer;

extern crate time;
extern crate serde;
extern crate exonum;
extern crate rustless;
extern crate valico;

use std::ops::Deref;
use std::marker::PhantomData;

use serde::{Serialize, Serializer};
use serde::de;
use serde::de::{Visitor, Deserialize, Deserializer};
use rustless::json::ToJson;
use rustless::{Api, Nesting};
use valico::json_dsl;

use exonum::crypto::{Hash, HexValue, ToHex};
use exonum::storage::{Error as StorageError};
use exonum::blockchain::{Blockchain};

pub use explorer::{TransactionInfo, BlockchainExplorer, BlockInfo};

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
                params.opt_typed("count", json_dsl::u64())
            });

            endpoint.handle(move |client, params| {
                let from = params.find("from").map(|x| x.as_u64().unwrap());
                let count = params.find("count").map(|x| x.as_u64().unwrap()).unwrap_or(100);

                let explorer = BlockchainExplorer::new(b1.clone());
                match explorer.blocks_range::<T>(count, from) {
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
                    Ok(None) => client.error(StorageError::new("Unable to find block with given height")),
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

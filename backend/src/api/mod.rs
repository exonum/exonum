#[cfg(test)]
mod tests;

use std::error::Error;

use router::Router;
use iron::prelude::*;
use params::{Params, Value};
use params;
use bodyparser;

use exonum::crypto::{Hash, HexValue, Signature};
use exonum::blockchain::Blockchain;
use exonum::node::TransactionSend;
use exonum::api::{Api, ApiError};
use exonum::storage::MapProof;

use {TimestampTx, TimestampingSchema, Content};

#[derive(Clone)]
pub struct PublicApi<T: TransactionSend + Clone> {
    channel: T,
    blockchain: Blockchain,
}

impl<T> PublicApi<T>
where
    T: TransactionSend + Clone,
{
    pub fn new(blockchain: Blockchain, channel: T) -> PublicApi<T> {
        PublicApi {
            blockchain: blockchain,
            channel: channel,
        }
    }

    fn put_content(&self, content: Content) -> Result<TimestampTx, ApiError> {
        {
            let hash = content.data_hash();
            let snapshot = self.blockchain.snapshot();

            if TimestampingSchema::new(&snapshot)
                .contents()
                .get(&hash)
                .is_some()
            {
                return Err(ApiError::FileExists(*hash));
            }
        }
        // Create transaction
        let tx = TimestampTx::new_with_signature(content, &Signature::zero());
        self.channel.send(Box::new(tx.clone()))?;
        Ok(tx)
    }

    fn get_content(&self, hash: &Hash) -> Result<Content, ApiError> {
        let view = self.blockchain.snapshot();
        TimestampingSchema::new(&view)
            .contents()
            .get(&hash)
            .ok_or_else(|| ApiError::FileNotFound(*hash))
    }

    fn get_proof(&self, hash_str: &str) -> Result<MapProof<Content>, ApiError> {
        let hash = Hash::from_hex(hash_str)?;
        let view = self.blockchain.snapshot();
        let schema = TimestampingSchema::new(&view);
        Ok(schema.contents().get_proof(&hash))
    }
}

fn find_str<'a>(map: &'a params::Map, path: &[&str]) -> Result<&'a str, ApiError> {
    let value = map.find(path);
    if let Some(&Value::String(ref s)) = value {
        Ok(s)
    } else {
        let msg = format!("Unable to find param: {:?}", path);
        Err(ApiError::IncorrectRequest(msg.into()))
    }
}

impl<T> Api for PublicApi<T>
where
    T: TransactionSend + Clone + 'static,
{
    // FIXME Rewrite without unwrap and boiler-plate code
    fn wire(&self, router: &mut Router) {
        // Receive a message by POST and play it back.
        let api = self.clone();
        let put_content = move |req: &mut Request| -> IronResult<Response> {
            match req.get::<bodyparser::Struct<Content>>() {
                Ok(Some(content)) => {
                    let tx = api.put_content(content)?;
                    api.ok_response(&json!(tx))
                }
                Ok(None) => Err(ApiError::IncorrectRequest("Empty request body".into()))?,
                Err(e) => Err(ApiError::IncorrectRequest(Box::new(e)))?,
            }
        };

        let api = self.clone();
        let get_content = move |req: &mut Request| -> IronResult<Response> {
            let map = req.get_ref::<Params>().unwrap();

            let hash = Hash::from_hex(find_str(map, &["hash"])?).map_err(|err| {
                ApiError::IncorrectRequest(err.description().into())
            })?;
            let content = api.get_content(&hash)?;

            api.ok_response(&json!(content))
        };

        let api = self.clone();
        let get_proof = move |req: &mut Request| -> IronResult<Response> {
            let map = req.get_ref::<Params>().unwrap();

            let hash = find_str(map, &["hash"])?;
            let proof = api.get_proof(&hash)?;

            api.ok_response(&json!(proof))
        };

        router.get("/v1/content/:hash", get_content, "get_content");
        router.get("/v1/proof/:hash", get_proof, "get_proof");
        router.put("/v1/content", put_content, "put_content");
    }
}

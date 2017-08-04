#[cfg(test)]
mod tests;

use iron::prelude::*;
use router::{Router, Params};
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

    fn get_proof(&self, hash: &Hash) -> Result<MapProof<Content>, ApiError> {
        let view = self.blockchain.snapshot();
        let schema = TimestampingSchema::new(&view);
        Ok(schema.contents().get_proof(&hash))
    }
}

fn parse_hex(map: &Params, id: &str) -> Result<Hash, ApiError> {
    match map.find(id) {
        Some(hex_str) => {
            let hash = Hash::from_hex(hex_str).map_err(|e| {
                let msg = format!(
                    "An error during parsing of the `{}` id occurred: {}",
                    hex_str,
                    e
                );
                ApiError::IncorrectRequest(msg.into())
            })?;
            Ok(hash)
        }
        None => {
            let msg = format!("The `{}` hash is not specified.", id);
            Err(ApiError::IncorrectRequest(msg.into()))
        }
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
            let map = req.extensions.get::<Router>().unwrap();

            let hash = parse_hex(&map, "hash")?;
            let content = api.get_content(&hash)?;

            api.ok_response(&json!(content))
        };

        let api = self.clone();
        let get_proof = move |req: &mut Request| -> IronResult<Response> {
            let map = req.extensions.get::<Router>().unwrap();

            let hash = parse_hex(&map, "hash")?;
            let proof = api.get_proof(&hash)?;

            api.ok_response(&json!(proof))
        };

        router.get("/v1/content/:hash", get_content, "get_content");
        router.get("/v1/proof/:hash", get_proof, "get_proof");
        router.put("/v1/content", put_content, "put_content");
    }
}

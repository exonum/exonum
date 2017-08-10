#[cfg(test)]
mod tests;

use iron::prelude::*;
use router::{Router, Params};
use bodyparser;
use serde::Deserialize;

use exonum::crypto::{Hash, HexValue, Signature};
use exonum::blockchain::{Blockchain, Transaction};
use exonum::node::TransactionSend;
use exonum::api::{Api, ApiError};
use exonum::storage::MapProof;

use blockchain::dto::{TxUpdateUser, TxTimestamp, TxPayment};

#[derive(Clone)]
pub struct PublicApi<T: TransactionSend + Clone + 'static> {
    channel: T,
    blockchain: Blockchain,
}

impl<T> PublicApi<T>
where
    T: TransactionSend + Clone + 'static,
{
    pub fn new(blockchain: Blockchain, channel: T) -> PublicApi<T> {
        PublicApi {
            blockchain: blockchain,
            channel: channel,
        }
    }

    // fn get_content(&self, hash: &Hash) -> Result<Content, ApiError> {
    //     let view = self.blockchain.snapshot();
    //     TimestampingSchema::new(&view)
    //         .contents()
    //         .get(&hash)
    //         .ok_or_else(|| ApiError::FileNotFound(*hash))
    // }

    // fn get_proof(&self, hash: &Hash) -> Result<MapProof<Content>, ApiError> {
    //     let view = self.blockchain.snapshot();
    //     let schema = TimestampingSchema::new(&view);
    //     Ok(schema.contents().get_proof(&hash))
    // }

    fn put_transaction<Tx: Transaction>(&self, tx: Tx) -> Result<Hash, ApiError> {
        let hash = tx.hash();
        self.channel.send(Box::new(tx))?;
        Ok(hash)
    }

    fn make_put_request<Tx>(&self, router: &mut Router, endpoint: &str, name: &str)
    where
        Tx: Transaction + Clone,
        for<'a> Tx: Deserialize<'a>,
    {
        let api = self.clone();
        let put_content = move |req: &mut Request| -> IronResult<Response> {
            match req.get::<bodyparser::Struct<Tx>>() {
                Ok(Some(tx)) => {
                    let hash = api.put_transaction(tx)?;
                    api.ok_response(&json!(hash))
                }
                Ok(None) => Err(ApiError::IncorrectRequest("Empty request body".into()))?,
                Err(e) => Err(ApiError::IncorrectRequest(Box::new(e)))?,
            }
        };
        router.put(endpoint, put_content, name);
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
    fn wire(&self, router: &mut Router) {
        self.make_put_request::<TxUpdateUser>(router, "/v1/users", "put_user");
        self.make_put_request::<TxPayment>(router, "/v1/payments", "put_payment");
        self.make_put_request::<TxTimestamp>(router, "/v1/timestamps", "put_timestamp");

        // Receive a message by POST and play it back.
        // let api = self.clone();
        // let put_content = move |req: &mut Request| -> IronResult<Response> {
        //     match req.get::<bodyparser::Struct<Content>>() {
        //         Ok(Some(content)) => {
        //             let tx = api.put_content(content)?;
        //             api.ok_response(&json!(tx))
        //         }
        //         Ok(None) => Err(ApiError::IncorrectRequest("Empty request body".into()))?,
        //         Err(e) => Err(ApiError::IncorrectRequest(Box::new(e)))?,
        //     }
        // };

        // let api = self.clone();
        // let get_content = move |req: &mut Request| -> IronResult<Response> {
        //     let map = req.extensions.get::<Router>().unwrap();

        //     let hash = parse_hex(&map, "hash")?;
        //     let content = api.get_content(&hash)?;

        //     api.ok_response(&json!(content))
        // };

        // let api = self.clone();
        // let get_proof = move |req: &mut Request| -> IronResult<Response> {
        //     let map = req.extensions.get::<Router>().unwrap();

        //     let hash = parse_hex(&map, "hash")?;
        //     let proof = api.get_proof(&hash)?;

        //     api.ok_response(&json!(proof))
        // };

        // router.get("/v1/content/:hash", get_content, "get_content");
        // router.get("/v1/proof/:hash", get_proof, "get_proof");
        // router.put("/v1/content", put_content, "put_content");
    }
}

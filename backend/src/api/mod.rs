mod error;

use router::Router;
use iron::prelude::*;
use iron::status;
use iron::mime::{Mime, TopLevel, SubLevel};
use params::{Params, Value};
use params;
use chrono::UTC;

use exonum::crypto::{Hash, HexValue, Signature};
use exonum::blockchain::Blockchain;
use exonum::storage::Map;
use exonum::node::TransactionSend;
use exonum::api::Api;

use {TimestampTx, TimestampingSchema, Content};
pub use self::error::Error as ApiError;

#[derive(Clone)]
pub struct PublicApi<T: TransactionSend + Clone> {
    channel: T,
    blockchain: Blockchain,
}

impl<T> PublicApi<T>
    where T: TransactionSend + Clone
{
    pub fn new(blockchain: Blockchain, channel: T) -> PublicApi<T> {
        PublicApi {
            blockchain: blockchain,
            channel: channel,
        }
    }

    fn put_content(&self, hash_str: &str, description: &str) -> Result<TimestampTx, ApiError> {
        let hash = Hash::from_hex(hash_str)?;
        let view = self.blockchain.view();

        if TimestampingSchema::new(&view)
               .contents()
               .get(&hash)?
               .is_some() {
            return Err(ApiError::FileExists(hash));
        }
        // Create transaction
        let ts = UTC::now().timestamp();
        let tx = TimestampTx::new_with_signature(&description, ts, &hash, &Signature::zero());
        self.channel.send(tx.clone())?;
        Ok(tx)
    }

    fn get_content(&self, hash_str: &str) -> Result<Content, ApiError> {
        let hash = Hash::from_hex(hash_str)?;
        let view = self.blockchain.view();
        TimestampingSchema::new(&view)
            .contents()
            .get(&hash)?
            .ok_or_else(|| ApiError::FileNotFound(hash))
    }
}

impl<T> Api for PublicApi<T>
    where T: TransactionSend + Clone + 'static
{
    fn wire(&self, router: &mut Router) {
        // Receive a message by POST and play it back.
        let api = self.clone();
        let put_content = move |req: &mut Request| -> IronResult<Response> {
            let map = req.get_ref::<Params>().unwrap();

            fn find_str<'a>(map: &'a params::Map, path: &[&str]) -> Result<&'a str, ApiError> {
                let value = map.find(path);
                if let Some(&Value::String(ref s)) = value {
                    Ok(s)
                } else {
                    Err(ApiError::IncorrectRequest)
                }
            };

            let hash = find_str(map, &["hash"])?;
            let description = find_str(map, &["description"]).unwrap_or("");

            let tx = api.put_content(hash, description)?;
            let content_type = Mime(TopLevel::Application, SubLevel::Json, Vec::new());
            let response = Response::with((content_type, status::Ok, json!(tx).to_string()));
            return Ok(response);
        };

        let api = self.clone();
        let get_content = move |req: &mut Request| -> IronResult<Response> {
            let ref hash = req.extensions
                .get::<Router>()
                .unwrap()
                .find("hash")
                .unwrap();
            let content = api.get_content(&hash)?;

            let content_type = Mime(TopLevel::Application, SubLevel::Json, Vec::new());
            let response = Response::with((content_type, status::Ok, json!(content).to_string()));
            Ok(response)
        };

        router.get("/v1/content/:hash", get_content, "get_content");
        router.post("/v1/content", put_content, "put_content");
    }
}

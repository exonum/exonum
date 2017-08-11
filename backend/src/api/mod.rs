#[cfg(test)]
mod tests;

use std::cmp;

use iron::prelude::*;
use router::{Router, Params as RouteParams};
use params::{Params, Value};
use bodyparser;
use serde::Deserialize;

use exonum::crypto::{Hash, HexValue};
use exonum::blockchain::{Blockchain, Transaction, BlockProof, Schema as CoreSchema};
use exonum::node::TransactionSend;
use exonum::api::{Api, ApiError};
use exonum::storage::MapProof;

use TIMESTAMPING_SERVICE;
use blockchain::ToHash;
use blockchain::schema::Schema;
use blockchain::dto::{TxUpdateUser, TxTimestamp, TxPayment, UserInfoEntry, TimestampEntry};

#[derive(Debug, Serialize)]
pub struct TimestampInfo {
    pub block_info: BlockProof,
    pub state_proof: MapProof<Hash>,
    pub user_proof: MapProof<UserInfoEntry>,
    pub timestamp_proof: MapProof<TimestampEntry>,
}

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

    pub fn put_transaction<Tx: Transaction>(&self, tx: Tx) -> Result<Hash, ApiError> {
        let hash = tx.hash();
        self.channel.send(Box::new(tx))?;
        Ok(hash)
    }

    pub fn user_info(&self, user_id: &str) -> Result<Option<UserInfoEntry>, ApiError> {
        let snap = self.blockchain.snapshot();
        Ok(Schema::new(&snap).users().get(&user_id.to_hash()))
    }

    pub fn timestamps_range(
        &self,
        user_id: &str,
        count: u64,
        upper: Option<u64>,
    ) -> Result<Vec<TimestampEntry>, ApiError> {
        let snap = self.blockchain.snapshot();
        let schema = ::blockchain::schema::Schema::new(&snap);
        let timestamps_history = schema.timestamps_history(user_id);
        let timestamps = schema.timestamps(user_id);

        let max_len = timestamps_history.len();
        let upper = upper.map(|x| cmp::min(x, max_len)).unwrap_or(max_len);
        let lower = upper.checked_sub(count).unwrap_or(0);
        let timestamps = (lower..upper)
            .rev()
            .map(|idx| {
                let key = timestamps_history.get(idx).unwrap();
                timestamps.get(&key).expect(&format!(
                    "Timestamp with key={:?} is absent in history table",
                    key
                ))
            })
            .collect::<Vec<_>>();
        Ok(timestamps)
    }

    pub fn timestamp_info(
        &self,
        user_id: &str,
        content_hash: &Hash,
    ) -> Result<TimestampInfo, ApiError> {
        let snap = self.blockchain.snapshot();
        let (state_proof, block_info) = {
            let core_schema = CoreSchema::new(&snap);

            let last_block_height = self.blockchain.last_block().height();
            let block_proof = core_schema.block_and_precommits(last_block_height).unwrap();
            let state_proof = core_schema.get_proof_to_service_table(TIMESTAMPING_SERVICE, 0);
            (state_proof, block_proof)
        };

        let schema = Schema::new(&snap);
        let user_proof = schema.users().get_proof(&user_id.to_hash());
        let timestamp_proof = schema.timestamps(user_id).get_proof(content_hash);

        Ok(TimestampInfo {
            block_info,
            state_proof,
            user_proof,
            timestamp_proof,
        })
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

fn parse_hex(map: &RouteParams, id: &str) -> Result<Hash, ApiError> {
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

fn parse_u64<S: AsRef<str>>(s: S) -> Result<u64, ApiError> {
    s.as_ref().parse::<u64>().map_err(|e| {
        let msg = format!("Unable to parse number, an error occured: {}", e);
        ApiError::IncorrectRequest(msg.into())
    })
}

impl<T> Api for PublicApi<T>
where
    T: TransactionSend + Clone + 'static,
{
    fn wire(&self, router: &mut Router) {
        let api = self.clone();
        let get_user = move |req: &mut Request| -> IronResult<Response> {
            let route = req.extensions.get::<Router>().unwrap();
            let id = route.find("user_id").ok_or_else(|| {
                let msg = "User id is unspecified";
                ApiError::IncorrectRequest(msg.into())
            })?;
            let user_info = api.user_info(id)?;
            api.ok_response(&json!(user_info))
        };

        let api = self.clone();
        let get_timestamp = move |req: &mut Request| -> IronResult<Response> {
            let route = req.extensions.get::<Router>().unwrap();
            let id = route.find("user_id").ok_or_else(|| {
                let msg = "User id is unspecified";
                ApiError::IncorrectRequest(msg.into())
            })?;
            let hash = parse_hex(route, "content_hash")?;
            let user_info = api.timestamp_info(id, &hash)?;
            api.ok_response(&json!(user_info))
        };

        let api = self.clone();
        let get_timestamp_range = move |req: &mut Request| -> IronResult<Response> {
            let user_id = {
                let route = req.extensions.get::<Router>().unwrap();
                route
                    .find("user_id")
                    .ok_or_else(|| {
                        let msg = "User id is unspecified";
                        ApiError::IncorrectRequest(msg.into())
                    })?
                    .to_owned()
            };

            let params = req.get_ref::<Params>().unwrap();
            let count = match params.find(&["count"]) {
                Some(&Value::String(ref count)) => parse_u64(count)?,
                _ => {
                    Err(ApiError::IncorrectRequest(
                        "Required parameter of timestamps 'count' is missing".into(),
                    ))?
                }
            };
            let from = match params.find(&["from"]) {
                Some(&Value::String(ref from)) => Some(parse_u64(from)?), 
                None => None,
                _ => {
                    Err(ApiError::IncorrectRequest(
                        "Parameter of timestamps 'from' is not number".into(),
                    ))?
                }
            };

            let timestamps = api.timestamps_range(&user_id, count, from)?;
            api.ok_response(&json!(timestamps))
        };

        self.make_put_request::<TxUpdateUser>(router, "/v1/users", "put_user");
        self.make_put_request::<TxPayment>(router, "/v1/payments", "put_payment");
        self.make_put_request::<TxTimestamp>(router, "/v1/timestamps", "put_timestamp");
        router.get("/v1/users/:user_id", get_user, "get_user");
        router.get(
            "/v1/timestamps/:user_id/:content_hash",
            get_timestamp,
            "get_timestamp",
        );
        router.get(
            "/v1/timestamps/:user_id",
            get_timestamp_range,
            "get_timestamp_range",
        );
    }
}

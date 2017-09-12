#[cfg(test)]
mod tests;
pub mod parser;

use std::cmp;

use iron::prelude::*;
use router::Router;
use bodyparser;
use serde::Deserialize;

use exonum::crypto::Hash;
use exonum::blockchain::{Blockchain, Transaction, BlockProof, Schema as CoreSchema};
use exonum::node::TransactionSend;
use exonum::api::{Api, ApiError};
use exonum::storage::MapProof;

use TIMESTAMPING_SERVICE;
use blockchain::ToHash;
use blockchain::schema::Schema;
use blockchain::dto::{TxUpdateUser, TxTimestamp, TxPayment, UserInfoEntry, TimestampEntry,
                      PaymentInfo};
use api::parser::RequestParser;

#[derive(Debug, Serialize)]
pub struct TimestampProof {
    pub block_info: BlockProof,
    pub state_proof: MapProof<Hash>,
    pub timestamp_proof: MapProof<TimestampEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ItemsTemplate<T> {
    pub total_count: u64,
    pub items: Vec<T>,
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
    ) -> Result<ItemsTemplate<TimestampEntry>, ApiError> {
        let snap = self.blockchain.snapshot();
        let schema = ::blockchain::schema::Schema::new(&snap);
        let timestamps_history = schema.timestamps_history(user_id);
        let timestamps = schema.timestamps();

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
        Ok(ItemsTemplate {
            items: timestamps,
            total_count: max_len,
        })
    }

    pub fn payments_range(
        &self,
        user_id: &str,
        count: u64,
        upper: Option<u64>,
    ) -> Result<ItemsTemplate<PaymentInfo>, ApiError> {
        let snap = self.blockchain.snapshot();
        let schema = ::blockchain::schema::Schema::new(&snap);
        let payments_history = schema.payments(user_id);

        let max_len = payments_history.len();
        let upper = upper.map(|x| cmp::min(x, max_len)).unwrap_or(max_len);
        let lower = upper.checked_sub(count).unwrap_or(0);
        // TODO use reverse iterators from storage
        let payments = (lower..upper)
            .rev()
            .map(|idx| payments_history.get(idx).unwrap())
            .collect::<Vec<_>>();
        Ok(ItemsTemplate {
            items: payments,
            total_count: max_len,
        })
    }

    pub fn users_range(
        &self,
        count: u64,
        upper: Option<u64>,
    ) -> Result<ItemsTemplate<UserInfoEntry>, ApiError> {
        let snap = self.blockchain.snapshot();
        let schema = ::blockchain::schema::Schema::new(&snap);
        let users_history = schema.users_history();
        let users = schema.users();

        let max_len = users_history.len();
        let upper = upper.map(|x| cmp::min(x, max_len)).unwrap_or(max_len);
        let lower = upper.checked_sub(count).unwrap_or(0);
        // TODO use reverse iterators from storage
        let users = (lower..upper)
            .rev()
            .map(|idx| {
                let key = users_history.get(idx).unwrap();
                users.get(&key).expect(&format!(
                    "User with hash_id={:?} is absent in history table",
                    key
                ))
            })
            .collect::<Vec<_>>();
        Ok(ItemsTemplate {
            items: users,
            total_count: max_len,
        })
    }

    pub fn timestamp_proof(&self, content_hash: &Hash) -> Result<TimestampProof, ApiError> {
        let snap = self.blockchain.snapshot();
        let (state_proof, block_info) = {
            let core_schema = CoreSchema::new(&snap);

            let last_block_height = self.blockchain.last_block().height();
            let block_proof = core_schema.block_and_precommits(last_block_height).unwrap();
            let state_proof = core_schema.get_proof_to_service_table(TIMESTAMPING_SERVICE, 0);
            (state_proof, block_proof)
        };

        let schema = Schema::new(&snap);
        let timestamp_proof = schema.timestamps().get_proof(content_hash);

        Ok(TimestampProof {
            block_info,
            state_proof,
            timestamp_proof,
        })
    }

    pub fn timestamp(&self, content_hash: &Hash) -> Result<Option<TimestampEntry>, ApiError> {
        let snap = self.blockchain.snapshot();
        let schema = ::blockchain::schema::Schema::new(&snap);
        Ok(schema.timestamps().get(content_hash))
    }

    fn make_post_request<Tx>(&self, router: &mut Router, endpoint: &str, name: &str)
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
        router.post(endpoint, put_content, name);
    }
}

impl<T> Api for PublicApi<T>
where
    T: TransactionSend + Clone + 'static,
{
    fn wire(&self, router: &mut Router) {
        let api = self.clone();
        let get_user = move |req: &mut Request| -> IronResult<Response> {
            let parser = RequestParser::new(req);
            let user_id = parser.route_param::<String>("user_id")?;

            let user_info = api.user_info(&user_id)?;
            api.ok_response(&json!(user_info))
        };

        let api = self.clone();
        let get_timestamp_proof = move |req: &mut Request| -> IronResult<Response> {
            let parser = RequestParser::new(req);
            let content_hash = parser.route_param("content_hash")?;

            let proof = api.timestamp_proof(&content_hash)?;
            api.ok_response(&json!(proof))
        };

        let api = self.clone();
        let get_timestamp = move |req: &mut Request| -> IronResult<Response> {
            let parser = RequestParser::new(req);
            let content_hash = parser.route_param("content_hash")?;

            let timestamp = api.timestamp(&content_hash)?;
            api.ok_response(&json!(timestamp))
        };

        let api = self.clone();
        let get_timestamps_range = move |req: &mut Request| -> IronResult<Response> {
            let mut parser = RequestParser::new(req);
            let user_id = parser.route_param::<String>("user_id")?;
            let count = parser.optional_param("count")?.ok_or_else(|| {
                ApiError::IncorrectRequest(
                    "Required parameter of timestamps 'count' is missing".into(),
                )
            })?;
            let from = parser.optional_param("from")?;

            let timestamps = api.timestamps_range(&user_id, count, from)?;
            api.ok_response(&json!(timestamps))
        };

        let api = self.clone();
        let get_payments_range = move |req: &mut Request| -> IronResult<Response> {
            let mut parser = RequestParser::new(req);
            let user_id = parser.route_param::<String>("user_id")?;
            let count = parser.optional_param("count")?.ok_or_else(|| {
                ApiError::IncorrectRequest(
                    "Required parameter of timestamps 'count' is missing".into(),
                )
            })?;
            let from = parser.optional_param("from")?;

            let payments = api.payments_range(&user_id, count, from)?;
            api.ok_response(&json!(payments))
        };

        let api = self.clone();
        let get_users_range = move |req: &mut Request| -> IronResult<Response> {
            let mut parser = RequestParser::new(req);
            let count = parser.optional_param("count")?.ok_or_else(|| {
                ApiError::IncorrectRequest("Required parameter of users 'count' is missing".into())
            })?;
            let from = parser.optional_param("from")?;

            let users = api.users_range(count, from)?;
            api.ok_response(&json!(users))
        };


        self.make_post_request::<TxUpdateUser>(router, "/v1/users", "post_user");
        self.make_post_request::<TxPayment>(router, "/v1/payments", "post_payment");
        self.make_post_request::<TxTimestamp>(router, "/v1/timestamps", "post_timestamp");
        router.get("/v1/users/:user_id", get_user, "get_user");
        router.get(
            "/v1/timestamps/value/:content_hash",
            get_timestamp,
            "get_timestamp",
        );
        router.get(
            "/v1/timestamps/proof/:content_hash",
            get_timestamp_proof,
            "get_timestamp_proof",
        );
        router.get(
            "/v1/timestamps/:user_id",
            get_timestamps_range,
            "get_timestamps_range",
        );
        router.get(
            "/v1/payments/:user_id",
            get_payments_range,
            "get_payments_range",
        );
        router.get("/v1/users", get_users_range, "get_users_range");
    }
}

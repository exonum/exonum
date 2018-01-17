extern crate core;

use serde_json::Value;
use serde::de::{Deserialize, DeserializeOwned, Deserializer};

use iron::prelude::*;
use iron::Handler;
use bodyparser;
use router::Router;

use serde_json;

use storage::{Fork, Snapshot};
use messages::RawTransaction;
use encoding::Error as MessageError;
use crypto::Hash;
use api::{ApiError, Api};
use node::{TransactionSend, ApiSender};


use super::{Transaction, ServiceContext, ApiContext, Service};
use std::marker::PhantomData;

/// TODO
pub trait TransactionSet: DeserializeOwned + Into<Box<Transaction>> + Clone {
    /// TODO
    fn tx_from_raw(raw: RawTransaction) -> Result<Box<Transaction>, MessageError>;
}

#[macro_export]
macro_rules! transaction_set {
    ( $name:ident { $($tx:ident),* } ) => {
        #[derive(Deserialize, Clone)]
        #[serde(untagged)] // :(
        pub enum $name {
            $($tx($tx),)*
        }

        impl $crate::blockchain::TransactionSet for $name {
            fn tx_from_raw(raw: RawTransaction) -> Result<Box<$crate::blockchain::Transaction>, $crate::encoding::Error> {
                let message_type = raw.message_type();
                $(
                    if $tx::message_id() == message_type {
                        return Ok(Box::new($tx::from_raw(raw)?));
                    }
                )*

                return Err($crate::encoding::Error::IncorrectMessageType { message_type })
            }
        }

        impl Into<Box<Transaction>> for $name {
            fn into(self) -> Box<Transaction> {
                match self {$(
                   $name::$tx(tx) => Box::new(tx),
                )*}
            }
        }
    }
}

/// The structure returned by the REST API.
#[derive(Serialize, Deserialize, Debug)]
pub struct TransactionResponse {
    /// TODO
    pub tx_hash: Hash,
}


/// TODO
pub trait Srv: Send + Sync + 'static {
    /// TODO
    type Transactions: TransactionSet;
    // = NoTransactions
    /// TODO
    const ID: u16;
    /// TODO
    const NAME: &'static str;

    /// TODOk
    fn state_hash(&self, snapshot: &Snapshot) -> Vec<Hash>;

    /// TODO
    fn initialize(&self, _fork: &mut Fork) -> Value {
        Value::Null
    }

    /// TODO
    fn handle_commit(&self, _context: &ServiceContext) {}

    /// TODO
    fn wire_public_api(&self, router: &mut Router, ctx: &ApiContext) {
    }

    /// TODO
    fn wire_private_api(&self, router: &mut Router, ctx: &ApiContext) {
    }

    /// TODO
    fn into_service(self) -> Box<Service> where Self: Sized {
        Box::new(SrvService(self))
    }
}



struct SrvService<S>(S);

impl<S: Srv> Service for SrvService<S> {
    fn service_id(&self) -> u16 {
        S::ID
    }

    fn service_name(&self) -> &'static str {
        S::NAME
    }

    fn state_hash(&self, snapshot: &Snapshot) -> Vec<Hash> {
        self.0.state_hash(snapshot)
    }

    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, MessageError> {
        S::Transactions::tx_from_raw(raw)
    }

    fn initialize(&self, fork: &mut Fork) -> Value {
        self.0.initialize(fork)
    }

    fn handle_commit(&self, context: &ServiceContext) {
        self.0.handle_commit(context)
    }

    fn public_api_handler(&self, context: &ApiContext) -> Option<Box<Handler>> {
        let api = SrvApi::<S> {
            srv: PhantomData,
            api_sender: context.node_channel().clone()
        };
        let mut router = Router::new();
        api.wire(&mut router);
        self.0.wire_public_api(&mut router, context);
        Some(Box::new(router))
    }

    fn private_api_handler(&self, context: &ApiContext) -> Option<Box<Handler>> {
        let mut router = Router::new();
        self.0.wire_private_api(&mut router, context);
        Some(Box::new(router))
    }
}

struct SrvApi<S> {
    srv: PhantomData<S>,
    api_sender: ApiSender,
}

impl<S> Clone for SrvApi<S> {
    fn clone(&self) -> Self {
        SrvApi { srv: PhantomData, api_sender: self.api_sender.clone() }
    }
}

impl<S: Srv> Api for SrvApi<S> {
    fn wire<'b>(&self, router: &'b mut Router) {
        let self_ = self.clone(); // TODO: whyyyy do we need this?
        router.post(
            "/transactions",
            move |req: &mut Request| -> IronResult<Response> {
                match req.get::<bodyparser::Struct<S::Transactions>>() {
                    Ok(None) => Err(ApiError::IncorrectRequest("Empty request body".into()))?,
                    Err(e) => Err(ApiError::IncorrectRequest(Box::new(e)))?,
                    Ok(Some(transaction)) => {
                        let transaction: Box<Transaction> = transaction.into();
                        let tx_hash = transaction.hash();
                        self_.api_sender.send(transaction).map_err(ApiError::from)?;
                        let json = TransactionResponse { tx_hash };
                        self_.ok_response(&serde_json::to_value(&json).unwrap())
                    }
                }
            },
            "transaction",
        );
    }
}

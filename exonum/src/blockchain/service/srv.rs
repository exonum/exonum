extern crate core;

use serde_json::Value;

use iron::prelude::*;
use iron::Handler;
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
pub trait TransactionSet {
    /// TODO
    fn tx_from_raw(raw: RawTransaction) -> Result<Box<Transaction>, MessageError>;
    /// TODO
    fn tx_from_request(request: &mut Request) -> Result<Box<Transaction>, ApiError>;
}

#[macro_export]
macro_rules! transaction_set {
    ( $name:ident { $($tx:ident),* } ) => {
        pub enum $name {
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

            //TODO: reexport & $crate::
            fn tx_from_request(request: &mut $crate::iron::Request) -> Result<Box<$crate::blockchain::Transaction>, $crate::api::ApiError> {
                use $crate::iron::prelude::*;

                #[derive(Deserialize, Clone)]
                #[serde(untagged)] // :(
                enum Any {
                    $($tx($tx),)*
                }

                match request.get::<$crate::bodyparser::Struct<Any>>() {
                    Ok(None) => Err($crate::api::ApiError::IncorrectRequest("Empty request body".into()))?,
                    Err(e) => Err($crate::api::ApiError::IncorrectRequest(Box::new(e)))?,
                    Ok(Some(any)) => {
                        match any {$(
                            Any::$tx(tx) => Ok(Box::new(tx)),
                        )*}
                    }
                }

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


/// Service capable of receiving and processing transactions
pub trait TransactionService: Send + Sync + 'static {
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
    fn handle_commit(&self, _context: &ServiceContext) {
    }

    /// TODO
    fn wire_public_api(&self, _router: &mut Router, _ctx: &ApiContext) {
    }

    /// TODO
    fn wire_private_api(&self, _router: &mut Router, _ctx: &ApiContext) {
    }

    /// TODO
    fn into_service(self) -> Box<Service> where Self: Sized {
        Box::new(TransactionServiceImpl(self))
    }
}

/// Service without transactions, which can observe blockchain state, but does not allow to
/// change it
pub trait ObserverService: Send + Sync + 'static {
    /// TODO
    const ID: u16;
    /// TODO
    const NAME: &'static str;

    /// TODO
    fn initialize(&self, _fork: &mut Fork) -> Value {
        Value::Null
    }

    /// TODO
    fn handle_commit(&self, _context: &ServiceContext) {
    }

    /// TODO
    fn wire_public_api(&self, _router: &mut Router, _ctx: &ApiContext) {
    }

    /// TODO
    fn wire_private_api(&self, _router: &mut Router, _ctx: &ApiContext) {
    }

    /// TODO
    fn into_service(self) -> Box<Service> where Self: Sized {
        struct ObserverServiceImpl<S>(S);

        impl<S: ObserverService> Service for ObserverServiceImpl<S> {
            fn service_id(&self) -> u16 { S::ID }
            fn service_name(&self) -> &'static str { S::NAME }
            fn state_hash(&self, _: &Snapshot) -> Vec<Hash> { Vec::new() }
            fn initialize(&self, fork: &mut Fork) -> Value { self.0.initialize(fork) }
            fn handle_commit(&self, context: &ServiceContext) { self.0.handle_commit(context) }

            fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, MessageError> {
                Err(MessageError::IncorrectMessageType { message_type: raw.message_type() })
            }

            fn public_api_handler(&self, context: &ApiContext) -> Option<Box<Handler>> {
                let mut router = Router::new();
                self.0.wire_public_api(&mut router, context);
                Some(Box::new(router))
            }

            fn private_api_handler(&self, context: &ApiContext) -> Option<Box<Handler>> {
                let mut router = Router::new();
                self.0.wire_private_api(&mut router, context);
                Some(Box::new(router))
            }
        }

        Box::new(ObserverServiceImpl(self))
    }
}



struct TransactionServiceImpl<S>(S);

impl<S: TransactionService> Service for TransactionServiceImpl<S> {
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
        let api = TransactionServiceImplApi::<S> {
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

struct TransactionServiceImplApi<S> {
    srv: PhantomData<S>,
    api_sender: ApiSender,
}

impl<S> Clone for TransactionServiceImplApi<S> {
    fn clone(&self) -> Self {
        TransactionServiceImplApi { srv: PhantomData, api_sender: self.api_sender.clone() }
    }
}

impl<S: TransactionService> Api for TransactionServiceImplApi<S> {
    fn wire<'b>(&self, router: &'b mut Router) {
        let self_ = self.clone(); // TODO: whyyyy do we need this?
        router.post(
            "/transactions",
            move |req: &mut Request| -> IronResult<Response> {
                let tx = S::Transactions::tx_from_request(req)?;
                let tx_hash = tx.hash();
                self_.api_sender.send(tx).map_err(ApiError::from)?;
                let json = TransactionResponse { tx_hash };
                self_.ok_response(&serde_json::to_value(&json).unwrap())
            },
            "transaction",
        );
    }
}

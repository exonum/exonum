//! Cryptocurrency implementation example using [exonum](http://exonum.com/).

extern crate serde;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate exonum;
extern crate bodyparser;
extern crate iron;
extern crate router;
#[macro_use]
extern crate failure;

pub use schema::CurrencySchema;

pub mod api;
pub mod schema;
pub mod transactions;
pub mod wallet;

use iron::Handler;
use router::Router;

use exonum::blockchain::{ApiContext, Service, Transaction, TransactionSet};
use exonum::crypto::Hash;
use exonum::encoding::Error as EncodingError;
use exonum::encoding::serialize::json::reexport as serde_json;
use exonum::helpers::fabric::{self, Context};
use exonum::messages::RawTransaction;
use exonum::storage::Snapshot;

use transactions::WalletTransactions;

/// Unique service ID.
const CRYPTOCURRENCY_SERVICE_ID: u16 = 128;
/// Initial balance of the wallet.
const INITIAL_BALANCE: u64 = 100;

/// Exonum `Service` implementation.
#[derive(Default, Debug)]
pub struct CurrencyService;

impl Service for CurrencyService {
    fn service_name(&self) -> &str {
        "cryptocurrency"
    }

    fn service_id(&self) -> u16 {
        CRYPTOCURRENCY_SERVICE_ID
    }

    fn state_hash(&self, view: &Snapshot) -> Vec<Hash> {
        let schema = CurrencySchema::new(view);
        schema.state_hash()
    }

    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, EncodingError> {
        WalletTransactions::tx_from_raw(raw).map(Into::into)
    }

    fn public_api_handler(&self, ctx: &ApiContext) -> Option<Box<Handler>> {
        let mut router = Router::new();
        use api;
        use exonum::api::Api;
        let api = api::CryptocurrencyApi {
            channel: ctx.node_channel().clone(),
            blockchain: ctx.blockchain().clone(),
        };
        api.wire(&mut router);
        Some(Box::new(router))
    }
}

pub struct ServiceFactory;

impl fabric::ServiceFactory for ServiceFactory {
    fn make_service(&mut self, _: &Context) -> Box<Service> {
        Box::new(CurrencyService)
    }
}

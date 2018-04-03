//! Cryptocurrency implementation example using [exonum](http://exonum.com/).

extern crate serde;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate exonum;
extern crate router;
extern crate iron;
extern crate bodyparser;
#[macro_use]
extern crate failure;

use iron::Handler;
use router::Router;

use exonum::messages::RawTransaction;
use exonum::crypto::Hash;
use exonum::storage::Snapshot;
use exonum::blockchain::{Service, Transaction, TransactionSet, ApiContext};
use exonum::encoding::serialize::json::reexport as serde_json;
use exonum::encoding::Error as EncodingError;
use exonum::helpers::fabric::{self, Context};

pub use schema::CurrencySchema;
use transactions::WalletTransactions;

pub mod api;
pub mod wallet;
pub mod transactions;
pub mod schema;

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

use iron::Handler;
use router::Router;

use exonum::api::Api;
use exonum::helpers::fabric::{ServiceFactory, Context};
use exonum::crypto::Hash;
use exonum::storage::Snapshot;
use exonum::blockchain::{Transaction, Service, ApiContext};
use exonum::messages::{FromRaw, RawTransaction};
use exonum::encoding::Error as MessageError;

use {TimestampingSchema, TimestampTx};
use api::PublicApi;

pub const TIMESTAMPING_SERVICE_ID: u16 = 128;

pub struct TimestampingService {}

impl TimestampingService {
    pub fn new() -> TimestampingService {
        TimestampingService {}
    }
}

impl Service for TimestampingService {
    fn service_id(&self) -> u16 {
        TIMESTAMPING_SERVICE_ID
    }

    fn service_name(&self) -> &'static str {
        "timestamping"
    }

    fn state_hash(&self, view: &Snapshot) -> Vec<Hash> {
        let schema = TimestampingSchema::new(view);
        schema.state_hash()
    }

    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, MessageError> {
        TimestampTx::from_raw(raw).map(|tx| Box::new(tx) as Box<Transaction>)
    }

    fn public_api_handler(&self, context: &ApiContext) -> Option<Box<Handler>> {
        let mut router = Router::new();
        let api = PublicApi::new(context.blockchain().clone(), context.node_channel().clone());
        api.wire(&mut router);
        Some(Box::new(router))
    }
}

impl ServiceFactory for TimestampingService {
    fn make_service(_: &Context) -> Box<Service> {
        Box::new(TimestampingService::new())
    }
}

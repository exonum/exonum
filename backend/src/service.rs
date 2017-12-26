use iron::Handler;
use router::Router;

use exonum::api::Api;
use exonum::helpers::fabric::{ServiceFactory, Context};
use exonum::crypto::Hash;
use exonum::storage::Snapshot;
use exonum::blockchain::{Transaction, Service, ApiContext};
use exonum::messages::RawTransaction;
use exonum::encoding::Error as StreamStructError;

use blockchain::dto::{TX_PAYMENT_ID, TX_TIMESTAMP_ID, TX_UPDATE_USER_ID, TxUpdateUser, TxPayment,
                      TxTimestamp};
use blockchain::schema::Schema;
use api::PublicApi;

pub const TIMESTAMPING_SERVICE: u16 = 128;

#[derive(Debug, Default)]
pub struct TimestampingService {}

impl TimestampingService {
    pub fn new() -> TimestampingService {
        TimestampingService {}
    }
}

impl Service for TimestampingService {
    fn service_id(&self) -> u16 {
        TIMESTAMPING_SERVICE
    }

    fn service_name(&self) -> &'static str {
        "timestamping"
    }

    fn state_hash(&self, view: &Snapshot) -> Vec<Hash> {
        let schema = Schema::new(view);
        schema.state_hash()
    }

    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, StreamStructError> {
        match raw.message_type() {
            TX_UPDATE_USER_ID => Ok(Box::new(TxUpdateUser::from_raw(raw)?)),
            TX_PAYMENT_ID => Ok(Box::new(TxPayment::from_raw(raw)?)),
            TX_TIMESTAMP_ID => Ok(Box::new(TxTimestamp::from_raw(raw)?)),
            _ => {
                Err(StreamStructError::IncorrectMessageType {
                    message_type: raw.message_type(),
                })
            }
        }
    }

    fn public_api_handler(&self, context: &ApiContext) -> Option<Box<Handler>> {
        let mut router = Router::new();
        let api = PublicApi::new(context.blockchain().clone(), context.node_channel().clone());
        api.wire(&mut router);
        Some(Box::new(router))
    }
}

impl ServiceFactory for TimestampingService {
    fn make_service(&mut self, _: &Context) -> Box<Service> {
        Box::new(TimestampingService::new())
    }
}

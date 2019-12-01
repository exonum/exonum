//! TODO

#![deny(
    unsafe_code,
    bare_trait_objects,
    //missing_docs,
    missing_debug_implementations
)]

pub use self::transactions::{Batch, CheckedCall, Error, UtilsInterface};

pub mod proto;
mod transactions;

use exonum::{
    blockchain::InstanceCollection,
    crypto::Hash,
    merkledb::Snapshot,
    runtime::{rust::Service, BlockchainData, InstanceId},
};
use exonum_derive::*;

#[derive(Debug, ServiceDispatcher, ServiceFactory)]
#[service_dispatcher(implements("UtilsInterface"))]
#[service_factory(proto_sources = "proto")]
pub struct UtilsService;

impl UtilsService {
    pub const DEFAULT_ID: InstanceId = 1;
    pub const DEFAULT_NAME: &'static str = "utils";
}

impl Service for UtilsService {
    fn state_hash(&self, _data: BlockchainData<&dyn Snapshot>) -> Vec<Hash> {
        vec![]
    }
}

impl From<UtilsService> for InstanceCollection {
    fn from(factory: UtilsService) -> Self {
        Self::new(factory).with_instance(UtilsService::DEFAULT_ID, UtilsService::DEFAULT_NAME, ())
    }
}

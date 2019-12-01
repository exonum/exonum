//! Utility service providing ways to compose transactions from the simpler building blocks.
//!
//! # Functionality overview
//!
//! ## Transaction batching
//!
//! [Batching] allows to atomically execute several transactions; if an error occurs
//! during execution, changes made by all transactions are rolled back. All transactions
//! in the batch are authorized in the same way as the batch itself.
//!
//! ## Checked call
//!
//! [Checked call] is a way to ensure that the called service corresponds to a specific artifact
//! with an expected version range. Unlike alternatives (e.g., finding out this information via
//! the `services` endpoint of the node HTTP API), using checked calls is most failsafe; by design,
//! it cannot suffer from [TOCTOU] issues. It does impose a certain overhead on the execution, though.
//!
//! [Batching]: trait.UtilsInterface.html#tymethod.batch
//! [Checked call]: trait.UtilsInterface.html#tymethod.checked_call
//! [TOCTOU]: https://en.wikipedia.org/wiki/Time-of-check_to_time-of-use

#![deny(
    unsafe_code,
    bare_trait_objects,
    missing_docs,
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

/// Utility service.
#[derive(Debug, ServiceDispatcher, ServiceFactory)]
#[service_dispatcher(implements("UtilsInterface"))]
#[service_factory(proto_sources = "proto")]
pub struct UtilsService;

impl UtilsService {
    /// Default numeric identifier of the utility service.
    pub const DEFAULT_ID: InstanceId = 1;
    /// Default name of the utility service.
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

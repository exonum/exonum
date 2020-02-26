// Copyright 2020 The Exonum Team
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Exonum explorer service.
//!
//! The explorer service does not define transactions, but it has several REST / WebSocket
//! endpoints allowing to retrieve information from the blockchain in a structured way.
//! Usually, the explorer service should be instantiated at the blockchain start
//! with the default identifiers. There may be no more than one explorer service on a blockchain;
//! an attempt to create a second service instance will lead to an error in the service
//! constructor.
//!
//! The API types necessary to interact with the service HTTP API are defined in a separate
//! crate, [`exonum-explorer`]. The base explorer provides Rust language APIs for retrieving info
//! from the blockchain, while this crate translates these APIs into REST and WebSocket endpoints
//! and packages this logic as an Exonum service. Thus, this crate is useful if you want to provide
//! the way for external apps to query the blockchain info.
//!
//! # HTTP API
//!
//! REST API of the service is documented in the [`api` module](api/index.html), and its
//! WebSocket API in the [`api::websocket` module](api/websocket/index.html).
//!
//! # Examples
//!
//! ## Use with Testkit
//!
//! ```
//! use exonum_explorer::api::BlocksRange;
//! use exonum_explorer_service::ExplorerFactory;
//! use exonum_testkit::{ApiKind, TestKit, TestKitBuilder};
//!
//! let mut testkit: TestKit = TestKitBuilder::validator()
//!     .with_default_rust_service(ExplorerFactory)
//!     // Add other services here
//!     .build();
//! // The explorer endpoints can be accessed via `api()`:
//! let api = testkit.api();
//! let BlocksRange { blocks, range } = api
//!     .public(ApiKind::Explorer)
//!     .get("v1/blocks?count=10")
//!     .unwrap();
//! ```
//!
//! [`exonum-explorer`]: https://docs.rs/exonum-explorer

#![deny(
    unsafe_code,
    bare_trait_objects,
    missing_docs,
    missing_debug_implementations
)]

use exonum::{
    merkledb::ObjectHash,
    runtime::{ExecutionContext, ExecutionError, ExecutionFail},
};
use exonum_derive::*;
use exonum_rust_runtime::{api::ServiceApiBuilder, AfterCommitContext, DefaultInstance, Service};

pub mod api;

use crate::api::{websocket::SharedState, ExplorerApi};

/// Errors that can occur during explorer service operation.
#[derive(Debug, Clone, Copy, ExecutionFail)]
pub enum Error {
    /// An explorer service is already instantiated on the blockchain.
    DuplicateExplorer = 0,
}

/// Explorer service.
#[derive(Debug, Default, ServiceDispatcher)]
pub struct ExplorerService {
    shared_state: SharedState,
}

impl Service for ExplorerService {
    fn initialize(
        &self,
        context: ExecutionContext<'_>,
        _params: Vec<u8>,
    ) -> Result<(), ExecutionError> {
        // Check that there are no other explorer services.
        let instances = context.data().for_dispatcher().service_instances();
        for instance in instances.values() {
            if instance.spec.artifact.name == env!("CARGO_PKG_NAME") {
                let msg = format!(
                    "An explorer service is already instantiated on the blockchain as {}",
                    instance.spec
                );
                return Err(Error::DuplicateExplorer.with_description(msg));
            }
        }
        Ok(())
    }

    fn after_commit(&self, context: AfterCommitContext<'_>) {
        let block_hash = context.data().for_core().last_block().object_hash();
        self.shared_state.broadcast_block(block_hash);
    }

    fn wire_api(&self, builder: &mut ServiceApiBuilder) {
        let blockchain = builder.blockchain().to_owned();
        let scope = builder
            .with_root_path(ExplorerFactory::INSTANCE_NAME)
            .public_scope();
        ExplorerApi::new(blockchain)
            .wire_rest(scope)
            .wire_ws(self.shared_state.get_ref(), scope);
    }
}

/// Explorer service factory.
#[derive(Debug, Clone, Copy, ServiceFactory)]
#[service_factory(service_constructor = "Self::new_instance")]
pub struct ExplorerFactory;

impl ExplorerFactory {
    #[allow(clippy::trivially_copy_pass_by_ref)]
    fn new_instance(&self) -> Box<dyn Service> {
        Box::new(ExplorerService::default())
    }
}

impl DefaultInstance for ExplorerFactory {
    const INSTANCE_ID: u32 = 2;
    const INSTANCE_NAME: &'static str = "explorer";
}

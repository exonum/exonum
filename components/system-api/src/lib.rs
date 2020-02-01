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

//! System API of an Exonum node, packaged as a node plugin.
//!
//! # HTTP API
//!
//! REST API of the service is documented in the corresponding modules:
//!
//! - [public API](public/index.html)
//! - [private API](private/index.html)
//!
//! # Examples
//!
//! Use with the real node builder:
//!
//! ```no_run
//! use exonum::{
//!     blockchain::config::GenesisConfig,
//!     keys::Keys,
//!     merkledb::TemporaryDB,
//! };
//! use exonum_node::{NodeBuilder, NodeConfig};
//! use exonum_system_api::SystemApiPlugin;
//!
//! let node_config: NodeConfig = // ...
//! #    unimplemented!();
//! let node_keys = Keys::random();
//! let genesis_config: GenesisConfig = // ...
//! #    unimplemented!();
//! let db = TemporaryDB::new();
//! let node = NodeBuilder::new(db, node_config, node_keys)
//!     .with_genesis_config(genesis_config)
//!     .with_plugin(SystemApiPlugin)
//!     // Add runtimes etc...
//!     .build();
//! node.run().unwrap();
//! ```
//!
//! Use with the testkit:
//!
//! ```
//! use exonum_system_api::{private::NodeInfo, SystemApiPlugin};
//! use exonum_testkit::{ApiKind, TestKitBuilder};
//!
//! let mut testkit = TestKitBuilder::validator()
//!     .with_plugin(SystemApiPlugin)
//!     .build();
//! let api = testkit.api();
//! let info: NodeInfo = api.private(ApiKind::System).get("v1/network").unwrap();
//! assert!(info.core_version.is_some());
//! ```
//!
//! Note that the testkit does not emulate the functionality of the node completely; it does
//! not update the `SharedNodeState`.

#![deny(
    unsafe_code,
    bare_trait_objects,
    missing_docs,
    missing_debug_implementations
)]

pub mod private;
pub mod public;

use exonum::blockchain::{ApiSender, Blockchain};
use exonum_api::ApiBuilder;
use exonum_node::{ExternalMessage, NodePlugin, PluginApiContext, SharedNodeState};

use crate::{private::SystemApi as PrivateSystemApi, public::SystemApi};

fn system_api(
    blockchain: Blockchain,
    sender: ApiSender<ExternalMessage>,
    shared_api_state: SharedNodeState,
) -> ApiBuilder {
    let mut builder = ApiBuilder::new();
    PrivateSystemApi::new(sender, shared_api_state.clone()).wire(builder.private_scope());
    SystemApi::new(blockchain, shared_api_state).wire(builder.public_scope());
    builder
}

/// Plugin responsible for adding system API to the Exonum node.
///
/// See crate docs for the examples of usage.
#[derive(Debug, Clone, Copy)]
pub struct SystemApiPlugin;

impl NodePlugin for SystemApiPlugin {
    fn wire_api(&self, context: PluginApiContext<'_>) -> Vec<(String, ApiBuilder)> {
        let api_builder = system_api(
            context.blockchain().to_owned(),
            context.api_sender(),
            context.node_state().to_owned(),
        );
        vec![("system".to_owned(), api_builder)]
    }
}

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
//! # async fn run_node() -> anyhow::Result<()> {
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
//! node.run().await?;
//! # Ok(())
//! # }
//! ```
//!
//! Use with the testkit:
//!
//! ```
//! use exonum_system_api::{private::{ConsensusStatus, NodeInfo}, SystemApiPlugin};
//! use exonum_testkit::{ApiKind, TestKitBuilder};
//!
//! # #[tokio::main]
//! # async fn main() -> anyhow::Result<()> {
//! let mut testkit = TestKitBuilder::validator()
//!     .with_plugin(SystemApiPlugin)
//!     .build();
//! let api = testkit.api();
//! let info: NodeInfo = api.private(ApiKind::System)
//!     .get("v1/info")
//!     .await?;
//! assert_eq!(info.consensus_status, ConsensusStatus::Enabled);
//! Ok(())
//! # }
//! ```
//!
//! Note that the testkit does not emulate the functionality of the node completely; it does
//! not update the `SharedNodeState`.

#![warn(
    missing_debug_implementations,
    missing_docs,
    unsafe_code,
    bare_trait_objects
)]
#![warn(clippy::pedantic)]
#![allow(
    // Next `cast_*` lints don't give alternatives.
    clippy::cast_possible_wrap, clippy::cast_possible_truncation, clippy::cast_sign_loss,
    // Next lints produce too much noise/false positives.
    clippy::module_name_repetitions, clippy::similar_names, clippy::must_use_candidate,
    clippy::pub_enum_variant_names,
    // '... may panic' lints.
    clippy::indexing_slicing,
    // Too much work to fix.
    clippy::missing_errors_doc
)]

pub mod private;

use exonum::blockchain::{ApiSender, Blockchain};
use exonum_api::ApiBuilder;
use exonum_node::{ExternalMessage, NodePlugin, PluginApiContext, SharedNodeState};

use crate::private::SystemApi;

fn system_api(
    blockchain: Blockchain,
    sender: ApiSender<ExternalMessage>,
    shared_api_state: SharedNodeState,
) -> ApiBuilder {
    let mut builder = ApiBuilder::new();
    SystemApi::new(blockchain, sender, shared_api_state).wire(builder.private_scope());
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

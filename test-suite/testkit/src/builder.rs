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

//! Testkit builder.

use exonum::{
    blockchain::config::GenesisConfigBuilder,
    crypto,
    helpers::ValidatorId,
    keys::Keys,
    merkledb::TemporaryDB,
    runtime::{RuntimeInstance, WellKnownRuntime},
};
#[cfg(feature = "exonum-node")]
use exonum_node::NodePlugin;
use exonum_rust_runtime::{spec::Deploy, RustRuntime, RustRuntimeBuilder};
use futures::channel::mpsc;

use std::net::SocketAddr;

use crate::{ApiNotifierChannel, TestKit, TestNetwork};

/// Builder for `TestKit`.
///
/// # Example
///
/// ```
/// # use exonum::{crypto::Hash, merkledb::Snapshot, runtime::BlockchainData};
/// # use exonum_derive::{exonum_interface, ServiceFactory, ServiceDispatcher};
/// # use exonum_testkit::{Spec, TestKitBuilder};
/// # use exonum_rust_runtime::{Service, ServiceFactory};
/// #
/// # const SERVICE_ID: u32 = 1;
/// #
/// # #[derive(Clone, Default, Debug, ServiceFactory, ServiceDispatcher)]
/// # #[service_factory(
/// #     artifact_name = "example",
/// #     artifact_version = "1.0.0",
/// # )]
/// # pub struct ExampleService;
/// # impl Service for ExampleService {}
/// #
/// let service = Spec::new(ExampleService).with_instance(SERVICE_ID, "example", ());
/// let mut testkit = TestKitBuilder::validator()
///     .with(service)
///     .with_validators(4)
///     .build();
/// testkit.create_block();
/// // Other test code
/// ```
#[derive(Debug)]
pub struct TestKitBuilder {
    our_validator_id: Option<ValidatorId>,
    test_network: Option<TestNetwork>,
    logger: bool,
    rust_runtime: RustRuntimeBuilder,
    api_notifier_channel: ApiNotifierChannel,
    additional_runtimes: Vec<RuntimeInstance>,
    #[cfg(feature = "exonum-node")]
    plugins: Vec<Box<dyn NodePlugin>>,
    genesis_config: GenesisConfigBuilder,
}

impl TestKitBuilder {
    /// Creates testkit for the validator node.
    pub fn validator() -> Self {
        Self::new(Some(ValidatorId(0)))
    }

    /// Creates testkit for the auditor node.
    pub fn auditor() -> Self {
        Self::new(None)
    }

    /// Creates the validator nodes from the specified keys.
    pub fn with_keys(mut self, keys: impl IntoIterator<Item = Keys>) -> Self {
        assert!(
            self.test_network.is_none(),
            "Number of validators is already specified"
        );
        self.test_network = Some(TestNetwork::with_our_role_from_keys(
            self.our_validator_id,
            keys,
        ));

        self
    }

    /// Sets the number of validator nodes in the test network.
    pub fn with_validators(mut self, validator_count: u16) -> Self {
        assert!(
            self.test_network.is_none(),
            "Number of validators is already specified"
        );
        self.test_network = Some(TestNetwork::with_our_role(
            self.our_validator_id,
            validator_count,
        ));

        self
    }

    /// Adds a deploy spec to this builder. The spec may contain artifacts and service instances
    /// to deploy at the blockchain start.
    pub fn with(mut self, spec: impl Deploy) -> Self {
        spec.deploy(&mut self.genesis_config, &mut self.rust_runtime);
        self
    }

    /// Adds a node plugin to the testkit.
    ///
    /// This method is only available if the crate is compiled with the `exonum-node` feature,
    /// which is off by default.
    #[cfg(feature = "exonum-node")]
    pub fn with_plugin(mut self, plugin: impl NodePlugin + 'static) -> Self {
        self.plugins.push(Box::new(plugin));
        self
    }

    /// Enables a logger inside the testkit.
    pub fn with_logger(mut self) -> Self {
        self.logger = true;
        self
    }

    /// Adds a runtime to the testkit in addition to the default Rust runtime.
    ///
    /// # Panics
    ///
    /// - Panics if the builder already contains specified runtime.
    pub fn with_additional_runtime(mut self, runtime: impl WellKnownRuntime) -> Self {
        let instance: RuntimeInstance = runtime.into();
        if instance.id == RustRuntime::ID
            || self.additional_runtimes.iter().any(|r| r.id == instance.id)
        {
            panic!(
                "TestkitBuilder already contains runtime with id {}",
                instance.id
            );
        }

        self.additional_runtimes.push(instance);
        self
    }

    /// Creates the testkit.
    pub fn build(mut self) -> TestKit {
        if self.logger {
            exonum::helpers::init_logger().ok();
        }
        crypto::init();

        let our_validator_id = self.our_validator_id;
        let network = self
            .test_network
            .unwrap_or_else(|| TestNetwork::with_our_role(our_validator_id, 1));

        let rust_runtime = self.rust_runtime.build(self.api_notifier_channel.0.clone());
        self.additional_runtimes.push(rust_runtime.into());
        let mut genesis_config = self.genesis_config.build();
        genesis_config.consensus_config = network.consensus_config();

        #[cfg(feature = "exonum-node")]
        {
            let mut testkit = TestKit::assemble(
                TemporaryDB::new(),
                network,
                Some(genesis_config),
                self.additional_runtimes,
                self.api_notifier_channel,
            );
            testkit.set_plugins(self.plugins);
            testkit
        }
        #[cfg(not(feature = "exonum-node"))]
        {
            TestKit::assemble(
                TemporaryDB::new(),
                network,
                Some(genesis_config),
                self.additional_runtimes,
                self.api_notifier_channel,
            )
        }
    }

    /// Starts a testkit web server, which listens to public and private APIs exposed by
    /// the testkit, on the respective addresses. The private address also exposes the testkit
    /// APIs with the `/api/testkit` URL prefix.
    ///
    /// Unlike real Exonum nodes, the testkit web server does not create peer-to-peer connections
    /// with other nodes, and does not create blocks automatically. The only way to commit
    /// transactions is thus to use the testkit API.
    ///
    /// See [`server` module](server/index.html) for the description of testkit server API.
    pub async fn serve(self, public_api_address: SocketAddr, private_api_address: SocketAddr) {
        let testkit = self.build();
        testkit.run(public_api_address, private_api_address).await
    }

    // Creates testkit for validator or auditor node.
    fn new(validator_id: Option<ValidatorId>) -> Self {
        let api_notifier_channel = mpsc::channel(16);
        Self {
            test_network: None,
            our_validator_id: validator_id,
            logger: false,
            rust_runtime: RustRuntimeBuilder::new(),
            api_notifier_channel,
            additional_runtimes: vec![],
            #[cfg(feature = "exonum-node")]
            plugins: vec![],
            genesis_config: GenesisConfigBuilder::default(),
        }
    }
}

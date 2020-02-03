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
    blockchain::config::{GenesisConfigBuilder, InstanceInitParams},
    crypto,
    helpers::ValidatorId,
    keys::Keys,
    merkledb::{BinaryValue, TemporaryDB},
    runtime::{migrations::MigrateData, ArtifactId, RuntimeInstance, WellKnownRuntime},
};
#[cfg(feature = "exonum-node")]
use exonum_node::NodePlugin;
use exonum_rust_runtime::{DefaultInstance, RustRuntime, RustRuntimeBuilder, ServiceFactory};
use futures::sync::mpsc;

use std::{collections::HashMap, net::SocketAddr};

use crate::{ApiNotifierChannel, TestKit, TestNetwork};

/// Builder for `TestKit`.
///
/// # Example
///
/// ```
/// # use exonum::{crypto::Hash, merkledb::Snapshot, runtime::BlockchainData};
/// # use exonum_derive::{exonum_interface, ServiceFactory, ServiceDispatcher};
/// # use exonum_testkit::TestKitBuilder;
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
/// # fn main() {
/// let service = ExampleService;
/// let artifact = service.artifact_id();
/// let mut testkit = TestKitBuilder::validator()
///     .with_artifact(artifact.clone())
///     .with_instance(artifact.into_default_instance(SERVICE_ID, "example"))
///     .with_rust_service(service)
///     .with_validators(4)
///     .build();
/// testkit.create_block();
/// // Other test code
/// # }
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
    instances: Vec<InstanceInitParams>,
    artifacts: HashMap<ArtifactId, Vec<u8>>,
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

    /// Adds a Rust service to the testkit.
    pub fn with_rust_service(mut self, service: impl ServiceFactory) -> Self {
        self.rust_runtime = self.rust_runtime.with_factory(service);
        self
    }

    /// Adds a Rust service with support of migrations to the testkit.
    ///
    /// # Stability
    ///
    /// This method is unstable because of instability of migration interfaces in the core crate.
    pub fn with_migrating_rust_service<S>(mut self, service: S) -> Self
    where
        S: ServiceFactory + MigrateData,
    {
        self.rust_runtime = self.rust_runtime.with_migrating_factory(service);
        self
    }

    /// Adds a Rust service that has default instance configuration to the testkit. Corresponding
    /// artifact and default instance are added implicitly.
    pub fn with_default_rust_service(self, service: impl DefaultInstance) -> Self {
        self.with_artifact(service.artifact_id())
            .with_instance(service.default_instance())
            .with_rust_service(service)
    }

    /// Adds a Rust service that has default instance configuration to the testkit. Corresponding
    /// artifact and default instance are added implicitly.
    ///
    /// # Stability
    ///
    /// This method is unstable because of instability of migration interfaces in the core crate.
    pub fn with_default_migrating_rust_service<S>(self, service: S) -> Self
    where
        S: DefaultInstance + MigrateData,
    {
        self.with_artifact(service.artifact_id())
            .with_instance(service.default_instance())
            .with_migrating_rust_service(service)
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

    /// Adds instances descriptions to the testkit that will be used for specification of builtin
    /// services of testing blockchain.
    pub fn with_instance(mut self, instance: impl Into<InstanceInitParams>) -> Self {
        self.instances.push(instance.into());
        self
    }

    /// Adds an artifact with no deploy argument. Does nothing in case artifact with given id is
    /// already added.
    pub fn with_artifact(self, artifact: impl Into<ArtifactId>) -> Self {
        self.with_parametric_artifact(artifact, ())
    }

    /// Adds an artifact with corresponding deploy argument. Does nothing in case artifact with
    /// given id is already added.
    pub fn with_parametric_artifact(
        mut self,
        artifact: impl Into<ArtifactId>,
        payload: impl BinaryValue,
    ) -> Self {
        let artifact = artifact.into();
        self.artifacts
            .entry(artifact)
            .or_insert_with(|| payload.into_bytes());
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
        let genesis = network.genesis_config();

        let rust_runtime = self.rust_runtime.build(self.api_notifier_channel.0.clone());
        self.additional_runtimes.push(rust_runtime.into());

        let genesis_config_builder = self.instances.into_iter().fold(
            GenesisConfigBuilder::with_consensus_config(genesis),
            |builder, instance| builder.with_instance(instance),
        );

        let genesis_config = self
            .artifacts
            .into_iter()
            .fold(genesis_config_builder, |builder, (artifact, payload)| {
                builder.with_parametric_artifact(artifact, payload)
            })
            .build();

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
    pub fn serve(self, public_api_address: SocketAddr, private_api_address: SocketAddr) {
        let testkit = self.build();
        testkit.run(public_api_address, private_api_address);
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
            instances: vec![],
            artifacts: HashMap::new(),
        }
    }
}

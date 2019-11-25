// Copyright 2019 The Exonum Team
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

pub use exonum::blockchain::InstanceCollection;

use exonum::{
    blockchain::config::{GenesisConfigBuilder, InstanceInitParams},
    crypto,
    helpers::ValidatorId,
    keys::Keys,
    merkledb::TemporaryDB,
    messages::BinaryValue,
    runtime::{
        rust::{DefaultInstance, RustRuntime, ServiceFactory},
        ArtifactId, Runtime,
    },
};
use futures::sync::mpsc;

use std::{collections::HashMap, net::SocketAddr};

use crate::{ApiNotifierChannel, TestKit, TestNetwork};

/// Builder for `TestKit`.
///
/// # Testkit server
///
/// By calling the [`serve`] method, you can transform testkit into a web server useful for
/// client-side testing. The testkit-specific APIs are exposed on the private address
/// with the `/api/testkit` prefix (hereinafter denoted as `{baseURL}`).
/// In all APIs, the request body (if applicable) and response are JSON-encoded.
///
/// ## Testkit status
///
/// GET `{baseURL}/v1/status`
///
/// Outputs the status of the testkit, which includes:
///
/// - Current blockchain height
/// - Current test network configuration
/// - Next network configuration if it is scheduled with [`commit_configuration_change`].
///
/// ## Create block
///
/// POST `{baseURL}/v1/blocks/create`
///
/// Creates a new block in the testkit blockchain. If the
/// JSON body of the request is an empty object, the call is functionally equivalent
/// to [`create_block`]. Otherwise, if the body has the `tx_hashes` field specifying an array
/// of transaction hashes, the call is equivalent to [`create_block_with_tx_hashes`] supplied
/// with these hashes.
///
/// Returns the latest block from the blockchain on success.
///
/// ## Roll back
///
/// POST `{baseURL}/v1/blocks/rollback`
///
/// Acts as a rough [`rollback`] equivalent. The blocks are rolled back up and including the block
/// at the specified in JSON body `height` value (a positive integer), so that after the request
/// the blockchain height is equal to `height - 1`. If the specified height is greater than the
/// blockchain height, the request performs no action.
///
/// Returns the latest block from the blockchain on success.
///
/// [`serve`]: #method.serve
/// [`create_block`]: struct.TestKit.html#method.create_block
/// [`create_block_with_tx_hashes`]: struct.TestKit.html#method.create_block_with_tx_hashes
/// [`commit_configuration_change`]: struct.TestKit.html#method.commit_configuration_change
/// [`rollback`]: struct.TestKit.html#method.rollback
///
/// # Example
///
/// ```
/// # use exonum_derive::{exonum_interface, ServiceFactory, ServiceDispatcher};
/// # use exonum_merkledb::Snapshot;
/// # use exonum_testkit::TestKitBuilder;
/// # use exonum::{
/// #     blockchain::InstanceCollection,
/// #     crypto::Hash,
/// #     runtime::{BlockchainData, rust::{InstanceInfoProvider, Service}},
/// # };
/// #
/// # const SERVICE_ID: u32 = 1;
/// #
/// # #[derive(Clone, Default, Debug, ServiceFactory, ServiceDispatcher)]
/// # #[service_dispatcher(implements("ExampleInterface"))]
/// # #[service_factory(
/// #     artifact_name = "example",
/// #     artifact_version = "1.0.0",
/// #     proto_sources = "exonum_testkit::proto",
/// # )]
/// # pub struct ExampleService;
/// #
/// # impl Service for ExampleService {
/// #     fn state_hash(&self, _: BlockchainData<&dyn Snapshot>) -> Vec<Hash> { vec![] }
/// # }
/// #
/// # #[exonum_interface]
/// # pub trait ExampleInterface {}
/// #
/// # impl ExampleInterface for ExampleService {}
/// #
/// # impl InstanceInfoProvider for ExampleService {}
/// #
/// # fn main() {
/// let service = ExampleService;
/// let mut testkit = TestKitBuilder::validator()
///     .with_artifact(service.get_artifact(), ())
///     .with_instance(service.get_instance(SERVICE_ID, "example", ()))
///     .with_rust_service(service)
///     .with_validators(4)
///     .create();
/// testkit.create_block();
/// // Other test code
/// # }
/// ```
#[derive(Debug)]
pub struct TestKitBuilder {
    our_validator_id: Option<ValidatorId>,
    test_network: Option<TestNetwork>,
    logger: bool,
    rust_runtime: RustRuntime,
    api_notifier_channel: ApiNotifierChannel,
    additional_runtimes: HashMap<u32, Box<dyn Runtime>>,
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
    pub fn with_rust_service(mut self, service: impl Into<Box<dyn ServiceFactory>>) -> Self {
        let service = service.into();
        self.rust_runtime = self.rust_runtime.with_factory(service);
        self
    }

    /// Adds a Rust service that has default instance configuration to the testkit. Corresponding
    /// artifact and default instance are added implicitly.
    pub fn with_rust_service_default(self, service: impl DefaultInstance) -> Self {
        self.with_artifact(service.get_artifact(), ())
            .with_instance(service.default_instance())
            .with_rust_service(service)
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
    /// - Panics if builder's instance already contains specified runtime.
    pub fn with_additional_runtime(mut self, runtime: impl Into<(u32, Box<dyn Runtime>)>) -> Self {
        let (id, runtime) = runtime.into();
        if id == RustRuntime::ID as u32 || self.additional_runtimes.contains_key(&id) {
            panic!("TestkitBuilder already contains runtime with id {}", id);
        }

        self.additional_runtimes.insert(id, runtime);
        self
    }

    /// Adds instances descriptions to the testkit that will be used for specification of builtin
    /// services of testing blockchain.
    pub fn with_instance(mut self, instance: InstanceInitParams) -> Self {
        self.instances.push(instance);
        self
    }

    /// Adds an artifact with corresponding deploy argument. Does nothing in case artifact with
    /// given id is already added.
    pub fn with_artifact(mut self, artifact: ArtifactId, deploy_args: impl BinaryValue) -> Self {
        if !self.artifacts.contains_key(&artifact) {
            self.artifacts.insert(artifact, deploy_args.into_bytes());
        }
        self
    }

    /// Creates the testkit.
    pub fn create(mut self) -> TestKit {
        if self.logger {
            exonum::helpers::init_logger().ok();
        }
        crypto::init();

        let our_validator_id = self.our_validator_id;
        let network = self
            .test_network
            .unwrap_or_else(|| TestNetwork::with_our_role(our_validator_id, 1));
        let genesis = network.genesis_config();

        let (id, runtime) = self.rust_runtime.into();
        self.additional_runtimes.insert(id, runtime);

        // Prepare GenesisConfig.
        let genesis_config_builder = self.instances.into_iter().fold(
            GenesisConfigBuilder::with_consensus_config(genesis),
            |builder, instance| builder.with_instance(instance),
        );

        let genesis_config = self
            .artifacts
            .into_iter()
            .fold(
                genesis_config_builder,
                |builder, (artifact, deploy_args)| builder.with_artifact(artifact, deploy_args),
            )
            .build();

        TestKit::assemble(
            TemporaryDB::new(),
            network,
            genesis_config,
            self.additional_runtimes.into_iter(),
            self.api_notifier_channel,
        )
    }

    /// Starts a testkit web server, which listens to public and private APIs exposed by
    /// the testkit, on the respective addresses. The private address also exposes the testkit
    /// APIs with the `/api/testkit` URL prefix.
    ///
    /// Unlike real Exonum nodes, the testkit web server does not create peer-to-peer connections
    /// with other nodes, and does not create blocks automatically. The only way to commit
    /// transactions is thus to use the [testkit API](#testkit-server).
    pub fn serve(self, public_api_address: SocketAddr, private_api_address: SocketAddr) {
        let testkit = self.create();
        testkit.run(public_api_address, private_api_address);
    }

    // Creates testkit for validator or auditor node.
    fn new(validator_id: Option<ValidatorId>) -> Self {
        let api_notifier_channel = mpsc::channel(16);
        Self {
            test_network: None,
            our_validator_id: validator_id,
            logger: false,
            rust_runtime: RustRuntime::new(api_notifier_channel.0.clone()),
            api_notifier_channel,
            additional_runtimes: HashMap::new(),
            instances: vec![],
            artifacts: HashMap::new(),
        }
    }
}

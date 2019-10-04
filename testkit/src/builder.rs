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

pub use exonum::blockchain::{InstanceCollection, InstanceConfig};

use exonum::{
    crypto,
    helpers::ValidatorId,
    keys::Keys,
    merkledb::TemporaryDB,
    runtime::{
        rust::{RustRuntime, ServiceFactory},
        Runtime,
    },
};

use std::net::SocketAddr;

use crate::{TestKit, TestNetwork};

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
/// ```ignore [ECR-3275]
/// # extern crate exonum;
/// # extern crate exonum_testkit;
/// # extern crate failure;
/// # use exonum::blockchain::{Service, Transaction};
/// # use exonum::messages::AnyTx;
/// # use exonum_testkit::TestKitBuilder;
/// # pub struct MyService;
/// # impl Service for MyService {
/// #    fn service_name(&self) -> &str {
/// #        "documentation"
/// #    }
/// #    fn state_hash(&self, _: &exonum_merkledb::Snapshot) -> Vec<exonum::crypto::Hash> {
/// #        Vec::new()
/// #    }
/// #    fn service_id(&self) -> u16 {
/// #        0
/// #    }
/// #    fn tx_from_raw(&self, _raw: AnyTx) -> Result<Box<Transaction>, failure::Error> {
/// #        unimplemented!();
/// #    }
/// # }
/// # fn main() {
/// let mut testkit = TestKitBuilder::validator()
///     .with_service(MyService)
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
    rust_services: Vec<Box<dyn ServiceFactory>>,
    runtimes: Vec<(u32, Box<dyn Runtime>)>,
    instances: Vec<InstanceConfig>,
}

impl TestKitBuilder {
    /// Creates testkit for the validator node.
    pub fn validator() -> Self {
        TestKitBuilder {
            test_network: None,
            our_validator_id: Some(ValidatorId(0)),
            logger: false,
            rust_services: vec![],
            runtimes: vec![],
            instances: vec![],
        }
    }

    /// Creates testkit for the auditor node.
    pub fn auditor() -> Self {
        TestKitBuilder {
            test_network: None,
            our_validator_id: None,
            logger: false,
            rust_services: vec![],
            runtimes: vec![],
            instances: vec![],
        }
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

    /// Adds a Rust service to the testkit. This is an old way convenient method that creates Rust
    /// runtime implicitly and is applicable to Rust services only. Thus, it must not be used in
    /// conjunction with the `.with_runtime()` method having the RustRuntime instance as an argument.
    pub fn with_service(mut self, service: impl Into<InstanceCollection>) -> Self {
        let InstanceCollection { factory, instances } = service.into();
        self.rust_services.push(factory);
        self.instances.extend(instances);
        self
    }

    /// Enables a logger inside the testkit.
    pub fn with_logger(mut self) -> Self {
        self.logger = true;
        self
    }

    /// Adds a runtime to the testkit. Must not be used with the Rust runtime as argument in
    /// conjunction with the `.with_service()` method.
    pub fn with_runtime(mut self, runtime: impl Into<(u32, Box<dyn Runtime>)>) -> Self {
        self.runtimes.push(runtime.into());
        self
    }

    pub fn with_instances(mut self, instances: impl IntoIterator<Item = InstanceConfig>) -> Self {
        self.instances.extend(instances);
        self
    }

    /// Creates the testkit.
    pub fn create(self) -> TestKit {
        if self.logger {
            exonum::helpers::init_logger().ok();
        }
        crypto::init();

        let our_validator_id = self.our_validator_id;
        let network = self
            .test_network
            .unwrap_or_else(|| TestNetwork::with_our_role(our_validator_id, 1));
        let genesis = network.genesis_config();

        let mut runtimes = vec![];
        if !self.rust_services.is_empty() {
            // The old `.with_service()` API has been used so we have to create Rust runtime here.
            let mut rust_runtime = RustRuntime::new();
            for service in self.rust_services {
                rust_runtime = rust_runtime.with_available_service(service);
            }
            runtimes.push(rust_runtime.into());
        }
        runtimes.extend(self.runtimes);

        TestKit::assemble(
            TemporaryDB::new(),
            network,
            genesis,
            runtimes,
            self.instances,
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
}

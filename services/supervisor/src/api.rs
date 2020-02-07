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

//! HTTP API for the supervisor service. Supervisor API is divided into public and private
//! parts, with public part intended for unauthorized use, and private parts intended to be
//! used by network administrator for the Exonum blockchain configuration.
//!
//! # Table of Contents
//!
//! - Public API:
//!
//!     - [Obtaining consensus configuration](#obtaining-consensus-configuration)
//!     - [Obtaining pending configuration proposal](#obtaining-pending-configuration-proposal)
//!
//! - Private API:
//!
//!     - [Request to deploy an artifact](#request-to-deploy-an-artifact)
//!     - [Request service migration](#request-service-migration)
//!     - [Request to accept new configuration](#request-to-accept-new-configuration)
//!     - [Vote for configuration proposal](#vote-for-configuration-proposal)
//!     - [Obtain current configuration number](#obtain-current-configuration-number)
//!     - [Obtain current supervisor operating mode](#obtain-current-supervisor-operating-mode)
//!     - [Check the deployment status](#check-deployment-status)
//!     - [Check the migration status](#check-migration-status)
//!
//! # Public API
//!
//! ## Obtaining Consensus Configuration
//!
//! | Property    | Value |
//! |-------------|-------|
//! | Path        | `/api/services/supervisor/consensus-config` |
//! | Method      | GET   |
//! | Query type  | - |
//! | Return type | [`ConsensusConfig`] |
//!
//! Returns the current consensus configuration.
//!
//! [`ConsensusConfig`]: https://docs.rs/exonum/latest/exonum/blockchain/config/struct.ConsensusConfig.html
//!
//! ```
//! use exonum::blockchain::ConsensusConfig;
//! use exonum_rust_runtime::ServiceFactory;
//! use exonum_supervisor::Supervisor;
//! use exonum_testkit::{ApiKind, TestKitBuilder};
//!
//! # fn main() -> Result<(), failure::Error> {
//! let mut testkit = TestKitBuilder::validator()
//!     .with_rust_service(Supervisor)
//!     .with_artifact(Supervisor.artifact_id())
//!     .with_instance(Supervisor::simple())
//!     .build();
//!
//! let consensus_config: ConsensusConfig = testkit
//!     .api()
//!     .public(ApiKind::Service("supervisor"))
//!     .get("consensus-config")?;
//!
//! # Ok(())
//! # }
//! ```
//!
//! ## Obtaining Pending Configuration Proposal
//!
//! | Property    | Value |
//! |-------------|-------|
//! | Path        | `/api/services/supervisor/config-proposal` |
//! | Method      | GET   |
//! | Query type  | - |
//! | Return type | `Option<[ConfigProposalWithHash]>` |
//!
//! Returns the configuration proposal which is currently pending. Returns `None` if there is no
//! pending configuration at the moment.
//!
//! [ConfigProposalWithHash]: ../struct.ConfigProposalWithHash.html
//!
//! ```
//! # use exonum_rust_runtime::ServiceFactory;
//! # use exonum_testkit::{ApiKind, TestKitBuilder};
//! use exonum_supervisor::{ConfigProposalWithHash, Supervisor};
//!
//! # fn main() -> Result<(), failure::Error> {
//! let mut testkit = // Same as in previous example...
//! #     TestKitBuilder::validator()
//! #         .with_rust_service(Supervisor)
//! #         .with_artifact(Supervisor.artifact_id())
//! #         .with_instance(Supervisor::simple())
//! #         .build();
//!
//! let pending_proposal: Option<ConfigProposalWithHash> = testkit
//!     .api()
//!     .public(ApiKind::Service("supervisor"))
//!     .get("config-proposal")?;
//!
//! // Will be none, since we did not send a proposal.
//! assert!(pending_proposal.is_none());
//! # Ok(())
//! # }
//! ```
//!
//! # Private API
//!
//! ## Request to Deploy an Artifact
//!
//! | Property    | Value |
//! |-------------|-------|
//! | Path        | `/api/services/supervisor/deploy-artifact` |
//! | Method      | POST   |
//! | Body type   | [`DeployRequest`] |
//! | Return type | [`Hash`] |
//!
//! Requests the deployment of a certain artifact.
//!
//! Depending on the supervisor operating mode, it may be required to send such a request to
//! majority of nodes (in "decentralized" mode), or one request will be enough (in "simple" mode).
//!
//! After receiving a deployment request, supervisor creates a corresponding transaction, signs it
//! with node's keys and broadcasts the transaction within the network. The hash of broadcast
//! transaction is returned from the endpoint.
//!
//! For more details on deploy requests, see [crate documentation](../index.html).
//!
//! **Warning:** `DeployRequest` structure should be serialized using corresponding protobuf message,
//! and represented as a hexadecimal string.
//!
//! [`DeployRequest`]: ../struct.DeployRequest.html
//! [`Hash`]: https://docs.rs/exonum-crypto/latest/exonum_crypto/struct.Hash.html
//!
//! ```
//! use exonum::{crypto::Hash, helpers::Height, merkledb::BinaryValue};
//! use exonum_supervisor::{DeployRequest, Supervisor};
//! # use exonum_testkit::{ApiKind, TestKitBuilder};
//! # use exonum_rust_runtime::ServiceFactory;
//!
//! # use exonum_derive::*;
//! # use exonum_rust_runtime::Service;
//! #
//! # #[derive(Debug, ServiceFactory, ServiceDispatcher)]
//! # #[service_factory(artifact_name = "exonum.doc.SomeService", artifact_version = "0.1.0")]
//! # pub struct SomeService;
//! #
//! # impl Service for SomeService {}
//! #
//! # fn config_for_service() -> Vec<u8> {
//! #     Vec::new()
//! # }
//! #
//! # fn main() -> Result<(), failure::Error> {
//! let mut testkit = // Same as in previous example...
//! #     TestKitBuilder::validator()
//! #         .with_rust_service(Supervisor)
//! #         .with_artifact(Supervisor.artifact_id())
//! #         .with_instance(Supervisor::simple())
//! #         .build();
//!
//! // In this example, we will try to deploy `SomeService` artifact.
//! let deploy_request = DeployRequest {
//!     artifact: SomeService.artifact_id(),
//!     spec: config_for_service(),
//!     deadline_height: Height(10),
//! };
//!
//! // `deploy_request` will be automatically serialized to hexadecimal string.
//! let tx_hash: Hash = testkit
//!     .api()
//!     .private(ApiKind::Service("supervisor"))
//!     .query(&deploy_request)
//!     .post("deploy-artifact")?;
//!
//! let block = testkit.create_block();
//! let result = block[tx_hash].status();
//! # // Call `expect` for a better error reporting if test will fail.
//! # result.expect("Deploy request failed");
//! assert!(result.is_ok());
//! # Ok(())
//! # }
//! ```
//!
//! ## Request Service Migration
//!
//! | Property    | Value |
//! |-------------|-------|
//! | Path        | `/api/services/supervisor/migrate` |
//! | Method      | POST   |
//! | Body type   | [`MigrationRequest`] |
//! | Return type | [`Hash`] |
//!
//! Requests the migration of certain service to a newer artifact version.
//!
//! Depending on the supervisor operating mode, it may be required to send such a request to
//! majority of nodes (in "decentralized" mode), or one request will be enough (in "simple" mode).
//!
//! After receiving a migration request, supervisor creates a corresponding transaction, signs it
//! with node's keys and broadcasts the transaction within the network. The hash of broadcast
//! transaction is returned from the endpoint.
//!
//! For more details on migration requests, see [crate documentation](../index.html).
//!
//! **Warning:** `MigrationRequest` structure should be serialized using corresponding protobuf message,
//! and represented as a hexadecimal string.
//!
//! [`MigrationRequest`]: ../struct.MigrationRequest.html
//! [`Hash`]: https://docs.rs/exonum-crypto/latest/exonum_crypto/struct.Hash.html
//!
//! ```
//! use exonum::crypto::Hash;
//! use exonum_supervisor::{MigrationRequest, Supervisor};
//! # use exonum::helpers::Height;
//! # use exonum_rust_runtime::ServiceFactory;
//! # use exonum_testkit::{ApiKind, TestKitBuilder};
//!
//! # fn main() -> Result<(), failure::Error> {
//! let mut testkit = TestKitBuilder::validator()
//!     .with_rust_service(Supervisor)
//!     .with_artifact(Supervisor.artifact_id())
//!     .with_instance(Supervisor::simple())
//!     // Add some service that supports migrations...
//!     .build();
//!
//! // Migration request creation skipped...
//! let migration_request = // Migration of some service.
//! #     // Request migration of supervisor for simplicity.
//! #     MigrationRequest {
//! #         new_artifact: Supervisor.artifact_id(),
//! #         service: Supervisor::NAME.to_owned(),
//! #         deadline_height: Height(10),
//! #     };
//!
//! // `migration_request` will be automatically serialized to hexadecimal string.
//! let tx_hash: Hash = testkit
//!     .api()
//!     .private(ApiKind::Service("supervisor"))
//!     .query(&migration_request)
//!     .post("migrate")?;
//!
//! let block = testkit.create_block();
//! let result = block[tx_hash].status();
//! assert!(result.is_ok());
//! # Ok(())
//! # }
//! ```
//!
//! ## Request to Accept New Configuration
//!
//! | Property    | Value |
//! |-------------|-------|
//! | Path        | `/api/services/supervisor/propose-config` |
//! | Method      | POST   |
//! | Body type   | [`ConfigPropose`] |
//! | Return type | [`Hash`] |
//!
//! Proposes the new configuration for the Exonum blockchain.
//!
//! Configuration changes may include the following items:
//!
//! - Request to change the consensus configuration.
//! - Request to start a new service instance.
//! - Request to stop an existing service instance.
//! - Request to resume a previously stopped service.
//! - Request to change the configuration of an existing service instance.
//!
//! Configuration proposal does not cause the configuration change itself, instead it
//! initializes a voting process: if node administrators of the network agree on the
//! suggested proposal, the configuration is applies. Otherwise, no changes in the
//! network configuration are performed.
//!
//! Voting for a configuration is performed via [`confirm-config`](#vote-for-configuration-proposal)
//! endpoint.
//!
//! For more details on configuration proposals, see [crate documentation](../index.html).
//!
//! **Warning:** `ConfigPropose` structure should be serialized using corresponding protobuf message,
//! and represented as a hexadecimal string.
//!
//! [`ConfigPropose`]: ../struct.ConfigPropose.html
//! [`Hash`]: https://docs.rs/exonum-crypto/latest/exonum_crypto/struct.Hash.html
//!
//! ```
//! use exonum::crypto::Hash;
//! use exonum_supervisor::{ConfigPropose, Supervisor};
//! # use exonum::helpers::Height;
//! # use exonum_rust_runtime::ServiceFactory;
//! # use exonum_testkit::{ApiKind, TestKitBuilder};
//!
//! # fn main() -> Result<(), failure::Error> {
//! let mut testkit = // Same as in previous example...
//! #     TestKitBuilder::validator()
//! #         .with_rust_service(Supervisor)
//! #         .with_artifact(Supervisor.artifact_id())
//! #         .with_instance(Supervisor::simple())
//! #         .build();
//!
//! let proposal: ConfigPropose = // Proposal creation skipped...
//! # ConfigPropose::new(0, Height(0));
//!
//! // `proposal` will be automatically serialized to hexadecimal string.
//! let tx_hash: Hash = testkit
//!     .api()
//!     .private(ApiKind::Service("supervisor"))
//!     .query(&proposal)
//!     .post("propose-config")?;
//!
//! // Create a block, so the proposal transaction will appear in the blockchain.
//! let block = testkit.create_block();
//!
//! // Verify that transaction was executed successfully.
//! assert!(block[tx_hash].status().is_ok());
//! # Ok(())
//! # }
//! ```
//!
//! ## Vote for Configuration Proposal
//!
//! | Property    | Value |
//! |-------------|-------|
//! | Path        | `/api/services/supervisor/confirm-config` |
//! | Method      | POST   |
//! | Query type  | [`DeployRequest`] |
//! | Return type | [`Hash`] |
//!
//! Votes for a pending configuration with a certain hash.
//!
//! Depending on the supervisor operating mode, it may be required to vote by majority of
//! nodes (in "decentralized" mode), or one vote will be enough (in "simple" mode).
//!
//! The node that broadcast the proposal is considered to have voted for it, there is
//! no need to send vote request for this node manually.
//!
//! After receiving a vote, supervisor creates a corresponding transaction, signs it
//! with node's keys and broadcasts the transaction within the network. The hash of broadcast
//! transaction is returned from the endpoint.
//!
//! For more details on voting, see [crate documentation](../index.html).
//!
//! **Warning:** `ConfigVote` structure should be serialized using corresponding protobuf message,
//! and represented as a hexadecimal string.
//!
//! [`ConfigVote`]: ../struct.ConfigVote.html
//! [`Hash`]: https://docs.rs/exonum-crypto/latest/exonum_crypto/struct.Hash.html
//!
//! ```
//! use exonum::crypto::Hash;
//! use exonum_supervisor::{ConfigPropose, ConfigVote, Supervisor, SupervisorInterface};
//! # use exonum::helpers::{Height, ValidatorId};
//! # use exonum::runtime::SUPERVISOR_INSTANCE_ID;
//! # use exonum_rust_runtime::ServiceFactory;
//! # use exonum_testkit::{ApiKind, TestKitBuilder};
//!
//! # fn main() -> Result<(), failure::Error> {
//! let mut testkit = // Same as in previous example (but with several validators)...
//! #     TestKitBuilder::validator()
//! #         .with_validators(2) // 2 validators to create a config to vote for.
//! #         .with_rust_service(Supervisor)
//! #         .with_artifact(Supervisor.artifact_id())
//! #         .with_instance(Supervisor::simple())
//! #         .build();
//! let proposal: ConfigPropose = // Proposal creation skipped...
//! # ConfigPropose::new(0, Height(10));
//!
//! // Assuming that config proposal was broadcast by other validator...
//! # let keys = testkit.validator(ValidatorId(1)).service_keypair();
//! # let tx = keys.propose_config_change(SUPERVISOR_INSTANCE_ID, proposal.clone());
//! # testkit.create_block_with_transaction(tx).transactions[0]
//! #     .status()
//! #     .expect("Transaction with change propose discarded.");
//! // Create a vote.
//! let config_vote = ConfigVote::from(proposal);
//!
//! // Send it.
//! // In this example, query is serialized to hexadecimal string automatically.
//! let tx_hash: Hash = testkit
//!     .api()
//!     .private(ApiKind::Service("supervisor"))
//!     .query(&config_vote)
//!     .post("confirm-config")?;
//!
//! // Create a block, so the proposal transaction will appear in the blockchain.
//! let block = testkit.create_block();
//! // Verify that transaction was executed successfully.
//! assert!(block[tx_hash].status().is_ok());
//! # Ok(())
//! # }
//! ```
//!
//! ## Obtain Current Configuration Number
//!
//! | Property    | Value |
//! |-------------|-------|
//! | Path        | `/api/services/supervisor/configuration-number` |
//! | Method      | GET   |
//! | Query type  | - |
//! | Return type | `u64` |
//!
//! To avoid the situation when several conflicting configuration proposals are broadcast
//! within the network, `ConfigPropose` contains a `configuration_number` field, which
//! should be equal to the amount of configurations, processed by supervisor (only configurations
//! that did participate in voting are counted, incorrect configurations are not).
//!
//! This field acts like a [nonce], approving the fact that node broadcasting proposal is
//! aware of the last accepted configuration.
//!
//! `configuration-number` endpoint allows requester to obtain the current number of processed
//! configurations.
//!
//! [nonce]: https://en.wikipedia.org/wiki/Cryptographic_nonce
//!
//! ```
//! use exonum_rust_runtime::ServiceFactory;
//! use exonum_supervisor::Supervisor;
//! # use exonum_testkit::{ApiKind, TestKitBuilder};
//!
//! # fn main() -> Result<(), failure::Error> {
//! let mut testkit = // Same as in previous example...
//! #     TestKitBuilder::validator()
//! #         .with_rust_service(Supervisor)
//! #         .with_artifact(Supervisor.artifact_id())
//! #         .with_instance(Supervisor::simple())
//! #         .build();
//!
//! let configuration_number: u64 = testkit
//!     .api()
//!     .private(ApiKind::Service("supervisor"))
//!     .get("configuration-number")?;
//!
//! // There was no configuration proposals, so configuration number is 0.
//! assert_eq!(configuration_number, 0);
//! # Ok(())
//! # }
//! ```
//!
//! ## Obtaining Supervisor Configuration
//!
//! | Property    | Value |
//! |-------------|-------|
//! | Path        | `/api/services/supervisor/supervisor-config` |
//! | Method      | GET   |
//! | Query type  | - |
//! | Return type | [`SupervisorConfig`] |
//!
//! Returns the current supervisor configuration, which includes the supervisor operating mode.
//!
//! [`SupervisorConfig`]: ../struct.SupervisorConfig.html
//!
//! ```
//! use exonum_rust_runtime::ServiceFactory;
//! use exonum_supervisor::{mode::Mode, Supervisor, SupervisorConfig};
//! use exonum_testkit::{ApiKind, TestKitBuilder};
//!
//! # fn main() -> Result<(), failure::Error> {
//! let mut testkit = // Same as in previous example...
//! #     TestKitBuilder::validator()
//! #         .with_rust_service(Supervisor)
//! #         .with_artifact(Supervisor.artifact_id())
//! #         .with_instance(Supervisor::simple())
//! #         .build();
//!
//! let config: SupervisorConfig = testkit
//!     .api()
//!     .private(ApiKind::Service("supervisor"))
//!     .get("supervisor-config")?;
//!
//! assert_eq!(config.mode, Mode::Simple);
//! # Ok(())
//! # }
//! ```
//!
//! ## Check the Deployment Status
//!
//! | Property    | Value |
//! |-------------|-------|
//! | Path        | `/api/services/supervisor/deploy-status` |
//! | Method      | GET   |
//! | Query type  | [`DeployInfoQuery`] |
//! | Return type | [`AsyncEventState`] |
//!
//! Returns the state of the deployment for a certain `ArtifactId`.
//!
//! [`DeployInfoQuery`]: struct.DeployInfoQuery.html
//! [`AsyncEventState`]: ../enum.AsyncEventState.html
//!
//! ```
//! # use exonum::{crypto::Hash, helpers::Height, merkledb::BinaryValue};
//! # use exonum_rust_runtime::ServiceFactory;
//! use exonum_supervisor::{
//!     api::DeployInfoQuery, DeployRequest, AsyncEventState, Supervisor,
//! };
//! # use exonum_testkit::{ApiKind, TestKitBuilder};
//!
//! # use exonum_derive::*;
//! # use exonum_rust_runtime::Service;
//! #
//! # #[derive(Debug, ServiceFactory, ServiceDispatcher)]
//! # #[service_factory(artifact_name = "exonum.doc.SomeService", artifact_version = "0.1.0")]
//! # pub struct SomeService;
//! #
//! # impl Service for SomeService {}
//! #
//! # fn main() -> Result<(), failure::Error> {
//! let mut testkit = // Same as in previous example...
//! #     TestKitBuilder::validator()
//! #         .with_rust_service(Supervisor)
//! #         .with_artifact(Supervisor.artifact_id())
//! #         .with_instance(Supervisor::simple())
//! #         .with_rust_service(SomeService)
//! #         .build();
//!
//! let deploy_request: DeployRequest = // Some previously performed deploy request.
//! #     DeployRequest {
//! #         artifact: SomeService.artifact_id(),
//! #         spec: Vec::new(),
//! #         deadline_height: Height(10),
//! #     };
//! # // Request deploy, so we will be able to request its state.
//! # let _hash: Hash = testkit
//! #     .api()
//! #     .private(ApiKind::Service("supervisor"))
//! #     .query(&deploy_request)
//! #     .post("deploy-artifact")?;
//! # testkit.create_block();
//! let query = DeployInfoQuery::from(deploy_request);
//!
//! let deploy_state: AsyncEventState = testkit
//!     .api()
//!     .private(ApiKind::Service("supervisor"))
//!     .query(&query)
//!     .get("deploy-status")?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Check the Migration Status
//!
//! | Property    | Value |
//! |-------------|-------|
//! | Path        | `/api/services/supervisor/migration-status` |
//! | Method      | GET   |
//! | Query type  | [`MigrationInfoQuery`] |
//! | Return type | [`MigrationState`] |
//!
//! Returns the state of the migration for a certain service instance.
//!
//! [`MigrationInfoQuery`]: struct.MigrationInfoQuery.html
//! [`MigrationState`]: ../struct.MigrationState.html
//!
//! ```
//! # use exonum::{crypto::Hash, helpers::Height, merkledb::BinaryValue};
//! # use exonum_rust_runtime::ServiceFactory;
//! use exonum_supervisor::{
//!     api::MigrationInfoQuery, MigrationRequest, MigrationState, Supervisor,
//! };
//! # use exonum_testkit::{ApiKind, TestKitBuilder};
//!
//! # fn main() -> Result<(), failure::Error> {
//! let mut testkit = // Same as in previous example...
//! #     TestKitBuilder::validator()
//! #         .with_rust_service(Supervisor)
//! #         .with_artifact(Supervisor.artifact_id())
//! #         .with_instance(Supervisor::simple())
//! #         .build();
//! let migration_request: MigrationRequest = // Some previously performed migration request.
//! #     MigrationRequest {
//! #         new_artifact: Supervisor.artifact_id(),
//! #         service: Supervisor::NAME.to_owned(),
//! #         deadline_height: Height(10),
//! #     };
//! # // Request migration. It will fail, but we'll be able to request its state.
//! # let _hash: Hash = testkit
//! #     .api()
//! #     .private(ApiKind::Service("supervisor"))
//! #     .query(&migration_request)
//! #     .post("migrate")?;
//! # testkit.create_block();
//! let query = MigrationInfoQuery::from(migration_request);
//!
//! let migration_state: MigrationState = testkit
//!     .api()
//!     .private(ApiKind::Service("supervisor"))
//!     .query(&query)
//!     .get("migration-status")?;
//! # Ok(())
//! # }
//! ```

use exonum::{blockchain::ConsensusConfig, crypto::Hash, helpers::Height, runtime::ArtifactId};
use exonum_rust_runtime::{
    api::{self, ServiceApiBuilder, ServiceApiState},
    Broadcaster,
};
use failure::Fail;
use serde_derive::{Deserialize, Serialize};

use std::convert::TryFrom;

use super::{
    schema::SchemaImpl, transactions::SupervisorInterface, AsyncEventState, ConfigProposalWithHash,
    ConfigPropose, ConfigVote, DeployRequest, MigrationRequest, MigrationState, SupervisorConfig,
};

/// Query for retrieving information about deploy state.
/// This is flattened version of `DeployRequest` which can be
/// encoded via URL query parameters.
#[derive(Debug, Clone, PartialEq)]
#[derive(Serialize, Deserialize)]
pub struct DeployInfoQuery {
    /// Artifact identifier as string, e.g. `0:exonum-supervisor:1.0.0-rc.1".
    pub artifact: String,
    /// Artifact spec bytes as hexadecimal string.
    pub spec: String,
    /// Deadline height.
    pub deadline_height: u64,
}

impl TryFrom<DeployInfoQuery> for DeployRequest {
    type Error = api::Error;

    fn try_from(query: DeployInfoQuery) -> Result<Self, Self::Error> {
        let artifact = query.artifact.parse::<ArtifactId>().map_err(|err| {
            api::Error::bad_request()
                .title("Invalid deploy request query")
                .detail(err.to_string())
        })?;
        let spec = hex::decode(query.spec).map_err(|err| {
            api::Error::bad_request()
                .title("Invalid deploy request query")
                .detail(err.to_string())
        })?;
        let deadline_height = Height(query.deadline_height);

        let request = Self {
            artifact,
            spec,
            deadline_height,
        };

        Ok(request)
    }
}

impl From<DeployRequest> for DeployInfoQuery {
    fn from(request: DeployRequest) -> Self {
        let artifact = request.artifact.to_string();
        let spec = hex::encode(&request.spec);
        let deadline_height = request.deadline_height.0;

        Self {
            artifact,
            spec,
            deadline_height,
        }
    }
}

/// Query for retrieving information about migration state.
/// This is flattened version of `MigrationRequest` which can be
/// encoded via URL query parameters.
#[derive(Debug, Clone, PartialEq)]
#[derive(Serialize, Deserialize)]
pub struct MigrationInfoQuery {
    /// Artifact identifier as string, e.g. `0:exonum-supervisor:1.0.0-rc.1"
    pub new_artifact: String,
    /// Target service name.
    pub service: String,
    /// Deadline height.
    pub deadline_height: u64,
}

impl TryFrom<MigrationInfoQuery> for MigrationRequest {
    type Error = api::Error;

    fn try_from(query: MigrationInfoQuery) -> Result<Self, Self::Error> {
        let new_artifact = query.new_artifact.parse::<ArtifactId>().map_err(|err| {
            api::Error::bad_request()
                .title("Invalid migration request query")
                .detail(err.to_string())
        })?;
        let deadline_height = Height(query.deadline_height);

        let request = Self {
            new_artifact,
            service: query.service,
            deadline_height,
        };

        Ok(request)
    }
}

impl From<MigrationRequest> for MigrationInfoQuery {
    fn from(request: MigrationRequest) -> Self {
        let new_artifact = request.new_artifact.to_string();
        let deadline_height = request.deadline_height.0;

        Self {
            new_artifact,
            service: request.service,
            deadline_height,
        }
    }
}

/// Private API specification of the supervisor service.
trait PrivateApi {
    /// Error type for the current API implementation.
    type Error: Fail;

    /// Creates and broadcasts the `DeployArtifact` transaction, which is signed
    /// by the current node, and returns its hash.
    fn deploy_artifact(&self, artifact: DeployRequest) -> Result<Hash, Self::Error>;

    /// Creates and broadcasts the `MigrationRequest` transaction, which is signed
    /// by the current node, and returns its hash.
    fn migrate(&self, request: MigrationRequest) -> Result<Hash, Self::Error>;

    /// Creates and broadcasts the `ConfigPropose` transaction, which is signed
    /// by the current node, and returns its hash.
    fn propose_config(&self, proposal: ConfigPropose) -> Result<Hash, Self::Error>;

    /// Creates and broadcasts the `ConfigVote` transaction, which is signed
    /// by the current node, and returns its hash.
    fn confirm_config(&self, vote: ConfigVote) -> Result<Hash, Self::Error>;

    /// Returns the number of processed configurations.
    fn configuration_number(&self) -> Result<u64, Self::Error>;

    /// Returns an actual supervisor config.
    fn supervisor_config(&self) -> Result<SupervisorConfig, Self::Error>;

    /// Returns the state of deployment for the given deploy request.
    fn deploy_status(&self, request: DeployInfoQuery) -> Result<AsyncEventState, Self::Error>;

    /// Returns the state of migration for the given migration request.
    fn migration_status(&self, request: MigrationInfoQuery) -> Result<MigrationState, Self::Error>;
}

trait PublicApi {
    /// Error type for the current API implementation.
    type Error: Fail;
    /// Returns an actual consensus configuration of the blockchain.
    fn consensus_config(&self) -> Result<ConsensusConfig, Self::Error>;
    /// Returns an pending propose config change.
    fn config_proposal(&self) -> Result<Option<ConfigProposalWithHash>, Self::Error>;
}

struct ApiImpl<'a>(&'a ServiceApiState<'a>);

impl ApiImpl<'_> {
    fn broadcaster(&self) -> Result<Broadcaster<'_>, api::Error> {
        self.0.broadcaster().ok_or_else(|| {
            api::Error::bad_request()
                .title("Invalid broadcast request")
                .detail("Nod is not a validator")
        })
    }
}

impl PrivateApi for ApiImpl<'_> {
    type Error = api::Error;

    fn deploy_artifact(&self, artifact: DeployRequest) -> Result<Hash, Self::Error> {
        self.broadcaster()?
            .request_artifact_deploy((), artifact)
            .map_err(|err| api::Error::internal(err).title("Artifact deploy request failed"))
    }

    fn migrate(&self, request: MigrationRequest) -> Result<Hash, Self::Error> {
        self.broadcaster()?
            .request_migration((), request)
            .map_err(|err| api::Error::internal(err).title("Migration start request failed"))
    }

    fn propose_config(&self, proposal: ConfigPropose) -> Result<Hash, Self::Error> {
        self.broadcaster()?
            .propose_config_change((), proposal)
            .map_err(|err| api::Error::internal(err).title("Config propose failed"))
    }

    fn confirm_config(&self, vote: ConfigVote) -> Result<Hash, Self::Error> {
        self.broadcaster()?
            .confirm_config_change((), vote)
            .map_err(|err| api::Error::internal(err).title("Config vote failed"))
    }

    fn configuration_number(&self) -> Result<u64, Self::Error> {
        let configuration_number =
            SchemaImpl::new(self.0.service_data()).get_configuration_number();
        Ok(configuration_number)
    }

    fn supervisor_config(&self) -> Result<SupervisorConfig, Self::Error> {
        let config = SchemaImpl::new(self.0.service_data()).supervisor_config();
        Ok(config)
    }

    fn deploy_status(&self, query: DeployInfoQuery) -> Result<AsyncEventState, Self::Error> {
        let request = DeployRequest::try_from(query)?;
        let schema = SchemaImpl::new(self.0.service_data());
        let status = schema.deploy_states.get(&request).ok_or_else(|| {
            Self::Error::not_found().title("No corresponding deploy request found")
        })?;

        Ok(status)
    }

    fn migration_status(&self, query: MigrationInfoQuery) -> Result<MigrationState, Self::Error> {
        let request = MigrationRequest::try_from(query)?;
        let schema = SchemaImpl::new(self.0.service_data());
        let status = schema.migration_states.get(&request).ok_or_else(|| {
            api::Error::not_found().title("No corresponding migration request found")
        })?;

        Ok(status)
    }
}

impl PublicApi for ApiImpl<'_> {
    type Error = api::Error;

    fn consensus_config(&self) -> Result<ConsensusConfig, Self::Error> {
        Ok(self.0.data().for_core().consensus_config())
    }

    fn config_proposal(&self) -> Result<Option<ConfigProposalWithHash>, Self::Error> {
        Ok(SchemaImpl::new(self.0.service_data())
            .public
            .pending_proposal
            .get())
    }
}

/// Wires Supervisor API endpoints.
pub(crate) fn wire(builder: &mut ServiceApiBuilder) {
    builder
        .private_scope()
        .endpoint_mut("deploy-artifact", |state, query| {
            ApiImpl(state).deploy_artifact(query)
        })
        .endpoint_mut("migrate", |state, query| ApiImpl(state).migrate(query))
        .endpoint_mut("propose-config", |state, query| {
            ApiImpl(state).propose_config(query)
        })
        .endpoint_mut("confirm-config", |state, query| {
            ApiImpl(state).confirm_config(query)
        })
        .endpoint("configuration-number", |state, _query: ()| {
            ApiImpl(state).configuration_number()
        })
        .endpoint("supervisor-config", |state, _query: ()| {
            ApiImpl(state).supervisor_config()
        })
        .endpoint("deploy-status", |state, query| {
            ApiImpl(state).deploy_status(query)
        })
        .endpoint("migration-status", |state, query| {
            ApiImpl(state).migration_status(query)
        });
    builder
        .public_scope()
        .endpoint("consensus-config", |state, _query: ()| {
            ApiImpl(state).consensus_config()
        })
        .endpoint("config-proposal", |state, _query: ()| {
            ApiImpl(state).config_proposal()
        });
}

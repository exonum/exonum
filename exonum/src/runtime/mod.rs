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

//! Transactions runtime.
//!
//! This module contains common building blocks for creating runtimes for the Exonum blockchain.
//!
//! Each runtime contains specific services to execute transactions, process events,
//! provide user APIs, etc. There is a unified dispatcher that redirects all the calls
//! and requests to the appropriate runtime environment. Thus, blockchain interacts with the
//! dispatcher, and not with a specific runtime instance.
//!
//! # Service life cycle
//!
//! 1. Each runtime has its own [artifacts] registry from which users can deploy them. The artifact
//! identifier is required by the runtime for constructing service instances. In other words,
//! an artifact identifier means same as class name, and a specific service instance is
//! the class instance.
//!
//! 2. Each validator administrator requests the dispatcher to deploy an artifact
//! and then validator node should send confirmation if this request is successful. Then, if the
//! number of confirmations is equal to the total number of validators, each validator calls the
//! dispatcher to register the artifact as deployed. After that validators can send requests to
//! start new services instances from this artifact.
//!
//! 3. To start a new service instance, each validator administrator should send request
//! to dispatcher. Each request contains the exactly same artifact identifier, instance name, and
//! instance configuration parameters. Then, as in the previous case, if the number of
//! confirmations is equal to the total number of validators, each validator calls dispatcher
//! to start a new service instance.
//!
//! 4. // TODO modify instance configuration procedure.
//!
//! 5. // TODO stop instance procedure.
//!
//! Each Exonum transaction is an [`AnyTx`] message with a verified/correct signature
//!
//! # Transaction life cycle
//!
//! 1. An Exonum client creates a transaction message which includes [CallInfo] information
//! about the corresponding handler and serialized transaction parameters as a payload;
//! and then signs the message with the author's key pair.
//!
//! 2. The client transmits the message to one of the Exonum nodes in the network.
//! The transaction is identified by the hash of the corresponding message.
//!
//! 3. Node verifies that transaction for a correctness of the signature and retransmits it to
//! other network nodes if it is correct.
//!
//! 4. When the validator decides to include transaction in the next block it takes the message
//! from the transaction pool and passes it to the [`Dispatcher`] for execution.
//!
//! 5. Dispatcher uses a lookup table to find the corresponding [`Runtime`] for the transaction
//! by the service [instance_id] recorded in the message. If the corresponding runtime is
//! successfully found, the dispatcher passes the transaction into found runtime for
//! immediate [execution].
//!
//! 6. After execution the transaction [execution status] is written into blockchain.
//!
//!
//! [`AnyTx`]: struct.AnyTx.html
//! [`CallInfo`]: struct.CallInfo.html
//! [`Dispatcher`]: dispatcher/struct.Dispatcher.html
//! [`instance_id`]: struct.CallInfo.html#structfield.instance_id
//! [`Runtime`]: trait.Runtime.html
//! [execution]: trait.Runtime.html#execute
//! [execution status]: error/struct.ExecutionStatus.html
//! [artifacts]: struct.ArtifactId.html
//!

pub use self::{
    error::{ErrorKind, ExecutionError},
    types::{AnyTx, ArtifactId, CallInfo, InstanceSpec, MethodId, ServiceInstanceId},
};

#[macro_use]
pub mod rust;
pub mod dispatcher;
pub mod error;
pub mod supervisor;

use exonum_merkledb::{Fork, Snapshot};
use futures::Future;

use std::fmt::Debug;

use crate::{
    api::ServiceApiBuilder,
    crypto::{Hash, PublicKey, SecretKey},
    node::ApiSender,
    proto::Any,
};

use self::dispatcher::{Dispatcher, DispatcherSender};

mod types;

/// List of well-known runtimes.
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
#[repr(u32)]
pub enum RuntimeIdentifier {
    /// Built-in Rust runtime.
    Rust = 0,
    /// Exonum Java Binding runtime.
    Java = 1,
}

impl From<RuntimeIdentifier> for u32 {
    fn from(id: RuntimeIdentifier) -> Self {
        id as Self
    }
}

/// This trait describes runtime environment for the Exonum services.
///
/// You can read more about the life cycle of services and transactions
/// [above](index.html#service-life-cycle).
///
/// Using this trait, you can extend the Exonum blockchain with the services written in
/// different languages. It assumes that the deployment procedure of a new service may be
/// complex and long and even may fail;
/// therefore, it was introduced an additional entity - artifacts.
/// Each artifact has a unique identifier and, depending on the runtime, may have an additional
/// specification which needs for its deployment. For example, the file to be compiled.
/// Artifact creates corresponding services instances, the same way as classes in object
/// oriented programming.
///
/// Please pay attention to the panic handling policy during the implementation of methods.
/// If no policy is specified, then the method should not panic and each panic will abort node.
///
/// Keep in mind that runtime methods can be executed in two ways: during the blocks execution
/// and during the node restart, thus be careful not to do unnecessary actions in the runtime
/// methods.
///
/// # Hints
///
/// * You may use [`catch_panic`](error/fn.catch_panic.html) method to catch panics in order of panic policy.
pub trait Runtime: Send + Debug + 'static {
    /// Request to deploy artifact with the given identifier and additional specification.
    ///
    /// # Policy on panics
    ///
    /// * This method should catch each kind of panics except of `FatalError` and converts
    /// them into `ExecutionError`.
    fn deploy_artifact(
        &mut self,
        artifact: ArtifactId,
        spec: Any,
    ) -> Box<dyn Future<Item = (), Error = ExecutionError>>;

    /// Returns protobuf description of deployed artifact with the specified identifier.
    fn artifact_info(&self, id: &ArtifactId) -> Option<ArtifactInfo>;

    /// Starts a new service instance with the given specification.
    ///
    /// # Policy on panics
    ///
    /// * This method should catch each kind of panics except of `FatalError` and converts
    /// them into `ExecutionError`.
    /// * If panic occurs, the runtime must ensure that it is in a consistent state.
    fn start_service(&mut self, spec: &InstanceSpec) -> Result<(), ExecutionError>;

    /// Configures a service instance with the given parameters.
    ///
    /// There are two cases when this method is called:
    ///
    /// - After creating a new service instance by the [`start_service`] invocation, in this case
    /// if an error during this action occurs, dispatcher will invoke [`stop_service`]
    /// and you must be sure that this invocation will not fail.
    /// - During the configuration change procedure. [ECR-3306]
    ///
    /// # Policy on panics
    ///
    /// * This method should catch each kind of panics except of `FatalError` and converts
    /// them into `ExecutionError`.
    ///
    /// ['start_service`]: #start_service
    /// ['stop_service`]: #stop_service
    fn configure_service(
        &self,
        fork: &Fork,
        spec: &InstanceSpec,
        parameters: Any,
    ) -> Result<(), ExecutionError>;

    /// Stops existing service instance with the given specification.
    ///
    /// # Policy on panics
    ///
    /// * This method should catch each kind of panics except of `FatalError` and converts
    /// them into `ExecutionError`.
    /// * If panic occurs, the runtime must ensure that it is in a consistent state.
    fn stop_service(&mut self, spec: &InstanceSpec) -> Result<(), ExecutionError>;

    /// Execute service transaction.
    ///
    /// # Policy on panics
    ///
    /// Do not process, just skip above.
    fn execute(
        &self,
        dispatcher: &dispatcher::Dispatcher,
        context: &mut ExecutionContext,
        call_info: CallInfo,
        payload: &[u8],
    ) -> Result<(), ExecutionError>;

    /// Gets state hashes of the every contained service.
    fn state_hashes(&self, snapshot: &dyn Snapshot) -> StateHashAggregator;

    /// Calls `before_commit` for all the services stored in the runtime.
    ///
    /// # Policy on panics
    ///
    /// * This method should catch each kind of panics except of `FatalError` and writes
    /// them into log.
    /// * If panic occurs, the runtime should rollback changes in fork.
    fn before_commit(&self, dispatcher: &Dispatcher, fork: &mut Fork);

    /// Calls `after_commit` for all the services stored in the runtime.
    ///
    /// # Policy on panics
    ///
    /// * This method should catch each kind of panics except of `FatalError` and writes
    /// them into log.
    fn after_commit(
        &self,
        dispatcher: &DispatcherSender,
        snapshot: &dyn Snapshot,
        service_keypair: &(PublicKey, SecretKey),
        tx_sender: &ApiSender,
    );

    fn services_api(&self) -> Vec<(String, ServiceApiBuilder)> {
        Vec::new()
    }
}

impl<T> From<T> for Box<dyn Runtime>
where
    T: Runtime,
{
    fn from(runtime: T) -> Self {
        Box::new(runtime) as Self
    }
}

/// Useful artifact information for Exonum clients.
#[derive(Debug, PartialEq)]
pub struct ArtifactInfo<'a> {
    /// List of protobuf files that make up the service interface, first element in tuple
    /// is the file name, second is its content.
    ///
    /// The common interface entry point is always in `service.proto` file.
    pub proto_sources: &'a [(&'a str, &'a str)],
}

impl<'a> Default for ArtifactInfo<'a> {
    /// Creates blank artifact information without any proto sources.
    fn default() -> Self {
        const EMPTY_SOURCES: [(&str, &str); 0] = [];

        Self {
            proto_sources: EMPTY_SOURCES.as_ref(),
        }
    }
}

/// An accessory structure that aggregates root objects hashes of runtime service
/// information schemas with the root hash of runtime information schema itself.
#[derive(Debug, PartialEq, Default)]
pub struct StateHashAggregator {
    /// List of hashes of the root objects of runtime information schemas.
    pub runtime: Vec<Hash>,
    /// List of hashes of the root objects of service instances schemas.
    pub instances: Vec<(ServiceInstanceId, Vec<Hash>)>,
}

/// The one who causes the transaction execution.
#[derive(Debug, PartialEq)]
pub enum Caller {
    /// A usual transaction from Exonum client, authorized by his key pair.
    Transaction {
        /// The transaction message hash.
        hash: Hash,
        /// Public key of the user who signed this transaction.
        author: PublicKey,
    },
    // This transaction is invoked on behalf of the blockchain itself,
    // for example [`before_commit`](trait.Runtime#before_commit) event.
    Blockchain,
}

impl Caller {
    /// Returns the author's public key, if it exists.
    pub fn author(&self) -> Option<PublicKey> {
        self.as_transaction().map(|(_hash, author)| *author)
    }

    /// Returns transaction hash, if it exists.
    pub fn transaction_hash(&self) -> Option<Hash> {
        self.as_transaction().map(|(hash, _)| *hash)
    }

    fn as_transaction(&self) -> Option<(&Hash, &PublicKey)> {
        if let Caller::Transaction { hash, author } = self {
            Some((hash, author))
        } else {
            None
        }
    }
}

/// Provides the current state of the blockchain and caller information for the transaction
/// which being executed.
#[derive(Debug)]
pub struct ExecutionContext<'a> {
    /// The current state of the blockchain. It includes the new, not-yet-committed, changes to
    /// the database made by the previous transactions already executed in this block.
    pub fork: &'a Fork,
    /// The one who causes the transaction execution.
    pub caller: Caller,
    actions: Vec<dispatcher::Action>,
}

impl<'a> ExecutionContext<'a> {
    pub(crate) fn new(fork: &'a Fork, caller: Caller) -> Self {
        Self {
            fork,
            caller,
            actions: Vec::new(),
        }
    }

    pub(crate) fn dispatch_action(&mut self, action: dispatcher::Action) {
        self.actions.push(action);
    }

    pub(crate) fn take_actions(&mut self) -> Vec<dispatcher::Action> {
        let mut other = Vec::new();
        std::mem::swap(&mut self.actions, &mut other);
        other
    }
}

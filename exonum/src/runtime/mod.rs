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
//! The module containing common building blocks for creating runtimes for the Exonum blockchain.
//! Each `Exonum` transaction is verified instance of [`AnyTx`] message.
//!
//! # Transaction life cycle
//!
//! 1. An Exonum client creates a transaction message, including [`CallInfo`] information to
//! find the corresponding handler to execute, serialized transaction parameters as a payload,
//! and signs the message with the author's key pair.
//!
//! 2. The client transmits the message to one of the Exonum nodes in the network.
//! The transaction is identified by the hash of the corresponding message.
//!
//! 3. Node verifies that the transaction has been correctly signed.
//!
//! 4. When the validator decides to include transaction in the next block it takes the message
//! from the transaction pool and passes it to the [`Dispatcher`] for execution.
//!
//! 5. Dispatcher uses a lookup table to find the corresponding [`Runtime`] for the transaction
//! by its [`instance_id`]. If the corresponding runtime is successfully found, the
//! dispatcher passes the transaction to it for immediate [execution].
//!
//! 6. After that the transaction [execution status] writes into blockchain.
//! 
//! Each runtime contains specific services to execute transactions, process events,
//! and provide user APIs, e.t.c. There is a unified dispatcher that redirects all calls
//! and requests to the appropriate runtime environment. Thus, users work with it, and not 
//! with a specific runtimes.
//! 
//! # Service life cycle
//! 
//! 1. Each runtime has own [artifacts] registry from which users can deploy them. The artifact
//! identifier is required by the runtime to construct service instances. In other words,
//! an artifact identifier means same as class name, and a specific service instance is
//! the class instance.
//! 
//! 2. Each validator should request the dispatcher to deploy an artifact and send confirmation 
//! if this request is successful. Then, if the number of confirmations is equal to the total 
//! number of validators, each validator calls the dispatcher to register the artifact as deployed.
//! After that validators can send requests to start new services instances from this artifact.
//! 
//! 3. To start a new service instance, each validator should send request to dispatcher. 
//! Each request contains the artifact identifier, instance name, and instance configuration parameters.
//! Then, as in the previous case, if the number of confirmations is equal to the total number of validators,
//! each validator calls dispatcher to start a new service instance.
//! 
//! 4. // TODO modify instance configuration procedure.
//! 
//! 5. // TODO stop instance procedure.
//! 
//! 
//!
//! [`AnyTx`]: struct.AnyTx.html
//! [`CallInfo`]: struct.CallInfo.html
//! [`Dispatcher`]: dispatcher/struct.Dispatcher.html
//! [`instance_id`]: struct.CallInfo.html#structfield.instance_id
//! [`Runtime`]: trait.Runtime.html
//! [`execution`]: trait.Runtime.html#execute
//! [execution status]: error/struct.ExecutionStatus.html
//! [artifacts]: struct.ArtifactId.html
//! 

pub use self::{
    error::ExecutionError,
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

/// List of well known runtimes.
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

/// Runtime environment for services.
///
/// It does not assign id to services/interfaces, ids are given to runtime from outside.
pub trait Runtime: Send + Debug + 'static {
    /// Request to deploy artifact with the given identifier and additional specification.
    /// It immediately returns true if artifact have already deployed.
    fn deploy_artifact(
        &mut self,
        artifact: ArtifactId,
        spec: Any,
    ) -> Box<dyn Future<Item = (), Error = ExecutionError>>;

    /// Returns additional information about artifact with the specified id if it is deployed.
    fn artifact_info(&self, id: &ArtifactId) -> Option<ArtifactInfo>;

    /// Starts a new service instance with the given specification.
    fn start_service(&mut self, spec: &InstanceSpec) -> Result<(), ExecutionError>;

    /// Configures a service instance with the given parameters.
    fn configure_service(
        &self,
        context: &Fork,
        spec: &InstanceSpec,
        parameters: Any,
    ) -> Result<(), ExecutionError>;

    /// Stops existing service instance with the given specification.
    fn stop_service(&mut self, spec: &InstanceSpec) -> Result<(), ExecutionError>;

    /// Execute transaction.
    // TODO Do not use dispatcher struct directly.
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
    fn before_commit(&self, dispatcher: &Dispatcher, fork: &mut Fork);

    // TODO interface should be re-worked
    /// Calls `after_commit` for all the services stored in the runtime.
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

#[derive(Debug, PartialEq)]
pub struct ArtifactInfo<'a> {
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

#[derive(Debug, PartialEq, Default)]
pub struct StateHashAggregator {
    pub runtime: Vec<Hash>,
    pub instances: Vec<(ServiceInstanceId, Vec<Hash>)>,
}

#[derive(Debug, PartialEq)]
pub enum Caller {
    Transaction { hash: Hash, author: PublicKey },
    Blockchain,
}

impl Caller {
    pub fn author(&self) -> Option<PublicKey> {
        self.as_transaction().map(|(_hash, author)| *author)
    }

    pub fn transaction_id(&self) -> Option<Hash> {
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

#[derive(Debug)]
pub struct ExecutionContext<'a> {
    pub fork: &'a Fork,
    pub caller: Caller,
    actions: Vec<dispatcher::Action>,
}

impl<'a> ExecutionContext<'a> {
    pub fn new(fork: &'a Fork, caller: Caller) -> Self {
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

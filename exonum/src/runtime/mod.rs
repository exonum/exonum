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

pub use self::error::ExecutionError;
pub use crate::messages::ServiceInstanceId;

use exonum_merkledb::{Fork, Snapshot};
use futures::Future;
use serde_derive::{Deserialize, Serialize};

use std::{
    fmt::{Debug, Display},
    str::FromStr,
};

use crate::{
    api::ServiceApiBuilder,
    crypto::{Hash, PublicKey, SecretKey},
    messages::CallInfo,
    node::ApiSender,
    proto::{schema, Any},
};

use self::dispatcher::{Dispatcher, DispatcherSender};

#[macro_use]
pub mod rust;
pub mod dispatcher;
pub mod error;
pub mod supervisor;

#[derive(Debug, Clone, PartialEq, Eq, Hash, ProtobufConvert, Serialize, Deserialize)]
#[exonum(pb = "schema::runtime::InstanceSpec", crate = "crate")]
pub struct InstanceSpec {
    pub id: ServiceInstanceId,
    pub artifact: ArtifactId,
    pub name: String,
}

// TODO Replace by more convenient solution [ECR-3222]
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
#[repr(u32)]
pub enum RuntimeIdentifier {
    Rust = 0,
    Java = 1,
}

impl From<RuntimeIdentifier> for u32 {
    fn from(id: RuntimeIdentifier) -> Self {
        id as u32
    }
}

#[derive(
    Debug, Clone, ProtobufConvert, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord,
)]
#[exonum(pb = "schema::runtime::ArtifactId", crate = "crate")]
pub struct ArtifactId {
    pub runtime_id: u32,
    pub name: String,
}

impl ArtifactId {
    /// Creates a new artifact identifier from the given runtime id and name.
    pub fn new(runtime_id: impl Into<u32>, name: impl Into<String>) -> Self {
        Self {
            runtime_id: runtime_id.into(),
            name: name.into(),
        }
    }
}

impl_binary_key_for_binary_value! { ArtifactId }

impl From<(String, u32)> for ArtifactId {
    fn from(v: (String, u32)) -> Self {
        Self {
            runtime_id: v.1,
            name: v.0,
        }
    }
}

impl Display for ArtifactId {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}:{}", self.runtime_id, self.name)
    }
}

impl FromStr for ArtifactId {
    type Err = failure::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let split = s.split(':').take(2).collect::<Vec<_>>();
        match &split[..] {
            [runtime_id, name] => Ok(Self {
                runtime_id: runtime_id.parse()?,
                name: name.to_string(),
            }),
            _ => Err(failure::format_err!(
                "Wrong artifact id format, it should be in form \"runtime_id:artifact_name\""
            )),
        }
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

#[test]
fn parse_artifact_id_correct() {
    ArtifactId::from_str("0:my-service/1.0.0").unwrap();
}

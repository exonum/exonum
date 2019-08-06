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

//! Building blocks for creating services' API.

pub use crate::api::{Error, FutureResult, Result};

use exonum_merkledb::{Database, Snapshot};
use futures::IntoFuture;
use serde::{de::DeserializeOwned, Serialize};

use std::sync::Arc;

use crate::{
    blockchain::Blockchain,
    crypto::{PublicKey, SecretKey},
    node::ApiSender,
    runtime::{InstanceDescriptor, InstanceId},
};

#[derive(Debug)]
pub struct ServiceApiState<'a> {
    service_keypair: (&'a PublicKey, &'a SecretKey),
    instance_descriptor: InstanceDescriptor<'a>,
    api_sender: &'a ApiSender,
    // TODO Think about avoiding of unnecessary snapshots creation. [ECR-3222]
    snapshot: Box<dyn Snapshot>,
}

impl<'a> ServiceApiState<'a> {
    pub fn instance(&self) -> InstanceDescriptor {
        self.instance_descriptor
    }

    /// Creates a read-only snapshot of the current blockchain state.
    pub fn snapshot(&'a self) -> &dyn Snapshot {
        self.snapshot.as_ref()
    }

    /// Returns the public key of the current node.
    pub fn public_key(&self) -> &PublicKey {
        self.service_keypair.0
    }

    /// Returns the secret key of the current node.
    pub fn secret_key(&self) -> &SecretKey {
        self.service_keypair.1
    }

    /// Returns a reference to the API sender.
    pub fn sender(&self) -> &ApiSender {
        self.api_sender
    }
}

// TODO rename [ECR-3222]
#[derive(Clone)]
pub struct ServiceApiContext {
    service_keypair: (PublicKey, SecretKey),
    api_sender: ApiSender,
    database: Arc<dyn Database>,
}

impl ServiceApiContext {
    pub fn with_blockchain(blockchain: &Blockchain) -> Self {
        Self {
            service_keypair: blockchain.service_keypair.clone(),
            api_sender: blockchain.api_sender.clone(),
            database: blockchain.db.clone(),
        }
    }

    pub fn state<'a>(&'a self, instance_descriptor: InstanceDescriptor<'a>) -> ServiceApiState<'a> {
        ServiceApiState {
            service_keypair: (&self.service_keypair.0, &self.service_keypair.1),
            instance_descriptor,
            api_sender: &self.api_sender,
            snapshot: self.snapshot(),
        }
    }

    fn snapshot(&self) -> Box<dyn Snapshot> {
        self.database.snapshot()
    }
}

#[derive(Clone)]
pub struct ServiceApiScope {
    inner: crate::api::ServiceApiScope,
    context: ServiceApiContext,
    descriptor: (InstanceId, String),
}

impl ServiceApiScope {
    pub fn new(context: ServiceApiContext, instance_descriptor: InstanceDescriptor) -> Self {
        Self {
            inner: crate::api::ServiceApiScope::new(),
            context,
            descriptor: instance_descriptor.into(),
        }
    }

    pub fn endpoint<Q, I, F, R>(&mut self, name: &'static str, handler: F) -> &mut Self
    where
        Q: DeserializeOwned + 'static,
        I: Serialize + 'static,
        F: Fn(&ServiceApiState, Q) -> R + 'static + Clone + Send + Sync,
        R: IntoFuture<Item = I, Error = crate::api::Error> + 'static,
    {
        let context = self.context.clone();
        let descriptor = self.descriptor.clone();
        self.inner.endpoint(
            name,
            move |_: &crate::api::ServiceApiState, query: Q| -> crate::api::FutureResult<I> {
                let state = context.state(InstanceDescriptor {
                    id: descriptor.0,
                    name: descriptor.1.as_ref(),
                });
                let result = handler(&state, query);
                Box::new(result.into_future())
            },
        );
        self
    }

    pub fn endpoint_mut<Q, I, F, R>(&mut self, name: &'static str, handler: F) -> &mut Self
    where
        Q: DeserializeOwned + 'static,
        I: Serialize + 'static,
        F: Fn(&ServiceApiState, Q) -> R + 'static + Clone + Send + Sync,
        R: IntoFuture<Item = I, Error = crate::api::Error> + 'static,
    {
        let context = self.context.clone();
        let descriptor = self.descriptor.clone();
        self.inner.endpoint_mut(
            name,
            move |_: &crate::api::ServiceApiState, query: Q| -> crate::api::FutureResult<I> {
                let mut state = context.state(InstanceDescriptor {
                    id: descriptor.0,
                    name: descriptor.1.as_ref(),
                });
                let result = handler(&mut state, query);
                Box::new(result.into_future())
            },
        );
        self
    }

    /// Returns a mutable reference to the underlying web backend.
    pub fn web_backend(&mut self) -> &mut crate::api::backends::actix::ApiBuilder {
        self.inner.web_backend()
    }
}

pub struct ServiceApiBuilder {
    public_scope: ServiceApiScope,
    private_scope: ServiceApiScope,
}

impl ServiceApiBuilder {
    /// Creates a new service API builder.
    pub(crate) fn new(context: ServiceApiContext, instance_descriptor: InstanceDescriptor) -> Self {
        Self {
            public_scope: ServiceApiScope::new(context.clone(), instance_descriptor),
            private_scope: ServiceApiScope::new(context, instance_descriptor),
        }
    }

    /// Returns a mutable reference to the public API scope builder.
    pub fn public_scope(&mut self) -> &mut ServiceApiScope {
        &mut self.public_scope
    }

    /// Returns a mutable reference to the private API scope builder.
    pub fn private_scope(&mut self) -> &mut ServiceApiScope {
        &mut self.private_scope
    }
}

impl From<ServiceApiBuilder> for crate::api::ServiceApiBuilder {
    fn from(inner: ServiceApiBuilder) -> Self {
        Self {
            public_scope: inner.public_scope.inner,
            private_scope: inner.private_scope.inner,
            blockchain: None,
        }
    }
}

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

//! Building blocks for creating HTTP API of Rust services.

pub use exonum_api::{Deprecated, EndpointMutability, Error, HttpStatusCode, Result};

use actix_web::{
    web::{Bytes, Json},
    FromRequest, HttpMessage,
};
use exonum::{
    blockchain::{Blockchain, Schema as CoreSchema},
    crypto::PublicKey,
    merkledb::{access::Prefixed, Snapshot},
    runtime::{
        ArtifactId, BlockchainData, InstanceDescriptor, InstanceState, InstanceStatus, SnapshotExt,
    },
};
use exonum_api::{backends::actix, ApiBackend, ApiBuilder, ApiScope, MovedPermanentlyError};
use exonum_proto::ProtobufConvert;
use futures::prelude::*;
use protobuf::Message;
use serde::{de::DeserializeOwned, Serialize};

use std::sync::Arc;

use super::Broadcaster;

/// Extracts request payload, which is encoded in either JSON or Protobuf.
async fn extract_pb_request<Q>(request: actix::HttpRequest, payload: actix::Payload) -> Result<Q>
where
    Q: DeserializeOwned + ProtobufConvert + 'static,
    Q::ProtoStruct: Message,
{
    match request.content_type() {
        "application/json" => Json::from_request(&request, &mut payload.into_inner())
            .await
            .map(Json::into_inner)
            .map_err(|err| {
                Error::bad_request()
                    .title("Cannot read JSON from request body")
                    .detail(err.to_string())
            }),

        "application/octet-stream" => {
            let bytes = Bytes::from_request(&request, &mut payload.into_inner())
                .await
                .map_err(|err| {
                    Error::bad_request()
                        .title("Cannot read Protobuf from request body")
                        .detail(err.to_string())
                })?;

            let mut message = <Q::ProtoStruct as Message>::new();
            message.merge_from_bytes(&bytes).map_err(|err| {
                Error::bad_request()
                    .title("Cannot parse Protobuf message")
                    .detail(err.to_string())
            })?;

            Q::from_pb(message).map_err(|err| {
                Error::bad_request()
                    .title("Cannot convert Protobuf message")
                    .detail(err.to_string())
            })
        }

        other => {
            let msg = format!(
                "Invalid content type: {}. Use `application/json` or `application/octet-stream`",
                other
            );
            Err(Error::bad_request()
                .title("Invalid content type")
                .detail(msg))
        }
    }
}

/// Provide the current blockchain state snapshot to API handlers.
///
/// This structure allows a service API handler to interact with the service instance
/// and other parts of the blockchain.
#[derive(Debug)]
pub struct ServiceApiState {
    /// Transaction broadcaster.
    broadcaster: Broadcaster,
    // TODO Think about avoiding of unnecessary snapshots creation. [ECR-3222]
    snapshot: Box<dyn Snapshot>,
    /// Endpoint path relative to the service root.
    endpoint: String,
    /// Current status of the service.
    status: InstanceStatus,
}

impl ServiceApiState {
    /// Creates service API context from the given blockchain and instance descriptor.
    fn new<S: Into<String>>(
        blockchain: &Blockchain,
        instance: InstanceDescriptor,
        expected_artifact: &ArtifactId,
        endpoint: S,
    ) -> Result<Self> {
        let snapshot = blockchain.snapshot();
        let instance_state = snapshot
            .for_dispatcher()
            .get_instance(instance.id)
            .ok_or_else(|| Self::removed_service_error(&instance))?;
        Self::check_service_artifact(&instance_state, expected_artifact)?;
        let status = instance_state
            .status
            .ok_or_else(|| Self::removed_service_error(&instance))?;

        Ok(Self {
            broadcaster: Broadcaster::new(
                instance,
                blockchain.service_keypair().clone(),
                blockchain.sender().clone(),
            ),
            snapshot,
            endpoint: endpoint.into(),
            status,
        })
    }

    fn removed_service_error(instance: &InstanceDescriptor) -> Error {
        let details = format!(
            "Service `{}` has been removed from the blockchain services, making it \
             impossible to process HTTP handlers",
            instance
        );
        Error::new(HttpStatusCode::INTERNAL_SERVER_ERROR)
            .title("Service is gone")
            .detail(details)
    }

    /// Returns readonly access to blockchain data.
    pub fn data(&self) -> BlockchainData<&dyn Snapshot> {
        BlockchainData::new(&self.snapshot, &self.instance().name)
    }

    /// Returns readonly access to the data of the executing service.
    pub fn service_data(&self) -> Prefixed<&dyn Snapshot> {
        self.data().for_executing_service()
    }

    /// Returns the access to the entire blockchain snapshot. Use [`data`](#method.data)
    /// or [`service_data`](#method.service_data) for more structure snapshot presentations.
    pub fn snapshot(&self) -> &dyn Snapshot {
        &self.snapshot
    }

    /// Returns the service key of this node.
    pub fn service_key(&self) -> PublicKey {
        self.broadcaster.keypair().public_key()
    }

    /// Returns information about the executing service.
    pub fn instance(&self) -> &InstanceDescriptor {
        self.broadcaster.instance()
    }

    /// Returns the current status of the service.
    pub fn status(&self) -> &InstanceStatus {
        &self.status
    }

    /// Returns a transaction broadcaster if the current node is a validator and the service
    /// is active (i.e., can process transactions). If these conditions do not hold, returns `None`.
    pub fn broadcaster(&self) -> Option<Broadcaster> {
        if self.status.is_active() {
            CoreSchema::new(&self.snapshot).validator_id(self.service_key())?;
            Some(self.broadcaster.clone())
        } else {
            None
        }
    }

    /// Returns a transaction broadcaster regardless of the node status (validator or auditor)
    /// and the service status (active or not).
    ///
    /// # Safety
    ///
    /// Transactions for non-active services will not be broadcast successfully; they will be
    /// filtered on the receiving nodes as ones that cannot (currently) be processed.
    pub fn generic_broadcaster(&self) -> Broadcaster {
        self.broadcaster.clone()
    }

    /// Creates a new builder for `MovedPermanently` response.
    pub fn moved_permanently(&self, new_endpoint: &str) -> MovedPermanentlyError {
        let new_url = Self::relative_to(&self.endpoint, new_endpoint);

        MovedPermanentlyError::new(new_url)
    }

    /// Takes an old endpoint and a new endpoint as direct URIs, and creates
    /// a relative path from the old to the new one.
    fn relative_to(old_endpoint: &str, new_endpoint: &str) -> String {
        let endpoint_without_end_slash = old_endpoint.trim_end_matches('/');
        let mut nesting_level = endpoint_without_end_slash
            .chars()
            .filter(|&c| c == '/')
            .count();

        // Mounting points do not contain the leading slash, e.g. `endpoint("v1/stats")`.
        nesting_level += 1;
        let path_to_service_root = "../".repeat(nesting_level);
        format!("{}{}", path_to_service_root, new_endpoint)
    }

    fn check_service_artifact(
        instance_state: &InstanceState,
        expected_artifact: &ArtifactId,
    ) -> Result<()> {
        let actual_artifact = instance_state.associated_artifact();
        if actual_artifact == Some(expected_artifact) {
            Ok(())
        } else {
            let details = format!(
                "Service `{}` was upgraded to version {}, making it impossible to continue \
                 using HTTP handlers from artifact `{}`. Depending on administrative actions, \
                 the server may be soon rebooted with updated endpoints",
                instance_state.spec.as_descriptor(),
                instance_state.data_version(),
                expected_artifact
            );
            Err(Error::new(HttpStatusCode::SERVICE_UNAVAILABLE)
                .title("Service has been upgraded, but its HTTP handlers are not rebooted yet")
                .detail(details))
        }
    }
}

/// Exonum API builder for the concrete service API scope.
#[derive(Debug, Clone)]
pub struct ServiceApiScope {
    inner: ApiScope,
    data: ScopeData,
}

#[derive(Debug, Clone)]
struct ScopeData {
    blockchain: Blockchain,
    descriptor: InstanceDescriptor,
    // Artifact associated with the service.
    artifact: ArtifactId,
}

impl ScopeData {
    fn wrap<F, I, Q, R>(
        &self,
        name: &str,
        handler: &F,
        query: Q,
    ) -> impl Future<Output = exonum_api::Result<I>>
    where
        F: Fn(ServiceApiState, Q) -> R + 'static,
        R: Future<Output = exonum_api::Result<I>>,
    {
        let maybe_state = ServiceApiState::new(
            &self.blockchain,
            self.descriptor.clone(),
            &self.artifact,
            name,
        );
        let state = match maybe_state {
            Ok(state) => state,
            Err(err) => return future::err(err).left_future(),
        };

        let descriptor = self.descriptor.clone();
        handler(state, query)
            .map_err(move |err| err.source(descriptor.to_string()))
            .right_future()
    }
}

impl ServiceApiScope {
    /// Creates a new service API scope for the specified service instance.
    fn new(blockchain: Blockchain, descriptor: InstanceDescriptor, artifact: ArtifactId) -> Self {
        Self {
            inner: ApiScope::new(),
            data: ScopeData {
                blockchain,
                descriptor,
                artifact,
            },
        }
    }

    /// Adds a readonly endpoint handler to the service API scope.
    ///
    /// In HTTP backends this type of endpoint corresponds to `GET` requests.
    pub fn endpoint<Q, I, F, R>(&mut self, name: &'static str, handler: F) -> &mut Self
    where
        Q: DeserializeOwned + 'static + Send,
        I: Serialize + 'static,
        F: Fn(ServiceApiState, Q) -> R + 'static + Clone + Send + Sync,
        R: Future<Output = exonum_api::Result<I>>,
    {
        let data = self.data.clone();
        self.inner
            .endpoint(name, move |query: Q| data.wrap(name, &handler, query));
        self
    }

    /// Adds an endpoint handler to the service API scope.
    ///
    /// In HTTP backends this type of endpoint corresponds to `POST` requests.
    pub fn endpoint_mut<Q, I, F, R>(&mut self, name: &'static str, handler: F) -> &mut Self
    where
        Q: DeserializeOwned + 'static,
        I: Serialize + 'static,
        F: Fn(ServiceApiState, Q) -> R + 'static + Clone + Send + Sync,
        R: Future<Output = exonum_api::Result<I>>,
    {
        let data = self.data.clone();
        self.inner
            .endpoint_mut(name, move |query: Q| data.wrap(name, &handler, query));
        self
    }

    /// Adds an endpoint handler to the service API scope.
    ///
    /// In HTTP backends this type of endpoint corresponds to `POST` requests.
    /// Unlike [`endpoint_mut`], this method supports deserializing the payload both from JSON
    /// (if `Content-Type` of the request is `application/json`) or from Protobuf
    /// (if `Content-Type` is `application/octet-stream`). This comes at the cost
    /// of more requirements to the response type.
    pub fn pb_endpoint_mut<Q, I, F, R>(&mut self, name: &'static str, handler: F) -> &mut Self
    where
        Q: DeserializeOwned + ProtobufConvert + 'static,
        Q::ProtoStruct: Message,
        I: Serialize + 'static,
        F: Fn(ServiceApiState, Q) -> R + 'static + Clone + Send + Sync,
        R: Future<Output = exonum_api::Result<I>>,
    {
        let data = self.data.clone();
        let raw_handler = move |http_request, payload| {
            let data = data.clone();
            let handler = handler.clone();

            async move {
                let query: Q = extract_pb_request(http_request, payload).await?;
                let response = data.wrap(name, &handler, query).await?;
                Ok(actix::HttpResponse::Ok().json(response))
            }
            .boxed_local()
        };
        let raw_handler = actix::RequestHandler {
            name: name.to_owned(),
            method: actix::HttpMethod::POST,
            inner: Arc::new(raw_handler),
        };
        self.inner.web_backend().raw_handler(raw_handler);
        self
    }

    /// Same as `endpoint`, but the response will contain a warning about the endpoint
    /// being deprecated. The endpoint expiration date and deprecation-related information
    /// (e.g., a link to documentation for a new API) can be included in the warning.
    pub fn deprecated_endpoint<Q, I, F, R>(
        &mut self,
        name: &'static str,
        deprecated: Deprecated<Q, I, R, F>,
    ) -> &mut Self
    where
        Q: DeserializeOwned + 'static,
        I: Serialize + 'static,
        F: Fn(ServiceApiState, Q) -> R + 'static + Clone + Send + Sync,
        R: Future<Output = exonum_api::Result<I>>,
    {
        let data = self.data.clone();
        let handler = deprecated.handler.clone();

        let full_handler = move |query: Q| data.wrap(name, &handler, query);
        // Mark endpoint as deprecated.
        let handler = deprecated.with_different_handler(full_handler);
        self.inner.endpoint(name, handler);
        self
    }

    /// Same as `endpoint_mut`, but the response will contain a warning about the endpoint
    /// being deprecated. The endpoint expiration date and deprecation-related information
    /// (e.g., a link to documentation for a new API) can be included in the warning.
    pub fn deprecated_endpoint_mut<Q, I, F, R>(
        &mut self,
        name: &'static str,
        deprecated: Deprecated<Q, I, R, F>,
    ) -> &mut Self
    where
        Q: DeserializeOwned + 'static,
        I: Serialize + 'static,
        F: Fn(ServiceApiState, Q) -> R + 'static + Clone + Send + Sync,
        R: Future<Output = exonum_api::Result<I>>,
    {
        let data = self.data.clone();
        let handler = deprecated.handler.clone();

        let full_handler = move |query: Q| data.wrap(name, &handler, query);
        // Mark endpoint as deprecated.
        let handler = deprecated.with_different_handler(full_handler);
        self.inner.endpoint_mut(name, handler);
        self
    }

    /// Return a mutable reference to the underlying web backend.
    pub fn web_backend(&mut self) -> &mut actix::ApiBuilder {
        self.inner.web_backend()
    }
}

/// Exonum service API builder which is used to add endpoints to the node API.
///
/// # Examples
///
/// The example below shows a common practice of the API implementation.
///
/// ```
/// use serde_derive::{Deserialize, Serialize};
/// use exonum::{blockchain::Schema, crypto::Hash, merkledb::ObjectHash};
/// use exonum_rust_runtime::api::{self, ServiceApiBuilder, ServiceApiState};
///
/// // Declare a type which describes an API specification and implementation.
/// pub struct MyApi;
///
/// // Declare structures for requests and responses.
///
/// // For the web backend `MyQuery` will be deserialized from the `block_height={number}` string.
/// #[derive(Deserialize, Clone, Copy)]
/// pub struct MyQuery {
///     pub block_height: u64,
/// }
///
/// // For the web backend `BlockInfo` will be serialized into a JSON string.
/// #[derive(Serialize, Clone, Copy)]
/// pub struct BlockInfo {
///     pub hash: Hash,
/// }
///
/// // Create API handlers.
/// impl MyApi {
///     /// Immutable handler which returns a hash of the block at the given height.
///     pub async fn block_hash(
///         state: ServiceApiState,
///         query: MyQuery,
///     ) -> api::Result<Option<BlockInfo>> {
///         let schema = state.data().for_core();
///         Ok(schema
///             .block_hashes_by_height()
///             .get(query.block_height)
///             .map(|hash| BlockInfo { hash }))
///     }
///
///     /// Simple handler without any parameters.
///     pub async fn ping(_state: ServiceApiState, _query: ()) -> api::Result<()> {
///         Ok(())
///     }
///
///     /// You may also use asynchronous tasks.
///     pub async fn async_operation(
///         _state: ServiceApiState,
///         query: MyQuery,
///     ) -> api::Result<Option<Hash>> {
///         # async fn long_async_task(query: MyQuery) -> Option<Hash> {
///         #     Some(Hash::zero())
///         # }
///         Ok(long_async_task(query).await)
///     }
/// }
///
/// fn wire_api(builder: &mut ServiceApiBuilder) -> &mut ServiceApiBuilder {
///     // Add `MyApi` handlers to the corresponding builder.
///     builder
///         .public_scope()
///         .endpoint("v1/block_hash", MyApi::block_hash)
///         .endpoint("v1/async_operation", MyApi::async_operation);
///     // Add a mutable endpoint to the private API.
///     builder
///         .private_scope()
///         .endpoint("v1/ping", MyApi::ping);
///     builder
/// }
/// ```
#[derive(Debug)]
pub struct ServiceApiBuilder {
    blockchain: Blockchain,
    public_scope: ServiceApiScope,
    private_scope: ServiceApiScope,
    root_path: Option<String>,
}

impl ServiceApiBuilder {
    /// Creates a new service API builder for the specified service instance.
    pub(crate) fn new(
        blockchain: Blockchain,
        instance: InstanceDescriptor,
        artifact: ArtifactId,
    ) -> Self {
        Self {
            blockchain: blockchain.clone(),
            public_scope: ServiceApiScope::new(
                blockchain.clone(),
                instance.clone(),
                artifact.clone(),
            ),
            private_scope: ServiceApiScope::new(blockchain, instance, artifact),
            root_path: None,
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

    /// Returns a reference to the blockchain.
    pub fn blockchain(&self) -> &Blockchain {
        &self.blockchain
    }

    /// Overrides the service root path as opposed to the default `services/$service_name`.
    ///
    /// # Safety
    ///
    /// The caller is responsible for the path not interfering with root paths of other services
    /// or that of the Rust runtime (`runtimes/rust`).
    #[doc(hidden)]
    pub fn with_root_path(&mut self, root_path: impl Into<String>) -> &mut Self {
        let root_path = root_path.into();
        self.root_path = Some(root_path);
        self
    }

    /// Takes the root path associated redefined by the service. If the service didn't redefine
    /// the root path, returns `None`.
    pub(super) fn take_root_path(&mut self) -> Option<String> {
        self.root_path.take()
    }
}

impl From<ServiceApiBuilder> for ApiBuilder {
    fn from(inner: ServiceApiBuilder) -> Self {
        Self {
            public_scope: inner.public_scope.inner,
            private_scope: inner.private_scope.inner,
        }
    }
}

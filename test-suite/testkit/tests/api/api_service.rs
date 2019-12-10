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

//! Service with API, but without any transactions.
//! This service can be used for testing features related only to the API.

use chrono::{TimeZone, Utc};
use serde_derive::{Deserialize, Serialize};

use exonum::{
    api::{self, EndpointMutability},
    runtime::{
        rust::{
            api::{ServiceApiBuilder, ServiceApiState},
            DefaultInstance, Service,
        },
        InstanceId,
    },
};
use exonum_derive::*;

pub const SERVICE_NAME: &str = "api-service";
pub const SERVICE_ID: InstanceId = 3;

#[exonum_interface]
pub trait ApiServiceInterface {}

impl ApiServiceInterface for ApiService {}

/// Sample query supported by API.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[derive(Serialize, Deserialize)]
pub struct PingQuery {
    pub value: u64,
}

#[derive(Debug, Clone, Copy)]
struct Api;

impl Api {
    /// Returns the same number that was in query.
    fn ping_pong(_state: &ServiceApiState<'_>, ping: PingQuery) -> api::Result<u64> {
        Ok(ping.value)
    }

    fn wire(builder: &mut ServiceApiBuilder) {
        let public_scope = builder.public_scope();

        // Normal endpoint.
        public_scope.endpoint("ping-pong", Self::ping_pong);

        // Deprecated endpoints.
        public_scope
            .deprecated_endpoint("ping-pong-deprecated", None, Self::ping_pong)
            .deprecated_endpoint(
                "ping-pong-deprecated-with-deadline",
                Some(Utc.ymd(2055, 12, 31)),
                Self::ping_pong,
            )
            .deprecated_endpoint_mut("ping-pong-deprecated-mut", None, Self::ping_pong);

        // Gone endpoints.
        public_scope
            .gone("gone-mutable", EndpointMutability::Mutable)
            .gone("gone-immutable", EndpointMutability::Immutable);

        // Moved endpoints.
        public_scope
            .moved_permanently("moved-mutable", "/ping-pong", EndpointMutability::Mutable)
            .moved_permanently(
                "moved-immutable",
                "/ping-pong",
                EndpointMutability::Immutable,
            );
    }
}

// // // // Service // // // //

#[derive(Debug, ServiceDispatcher, ServiceFactory)]
#[service_factory(artifact_name = "api-service", artifact_version = "1.0.0")]
#[service_dispatcher(implements("ApiServiceInterface"))]
pub struct ApiService;

impl DefaultInstance for ApiService {
    const INSTANCE_ID: u32 = SERVICE_ID;
    const INSTANCE_NAME: &'static str = SERVICE_NAME;
}

impl Service for ApiService {
    fn wire_api(&self, builder: &mut ServiceApiBuilder) {
        Api::wire(builder)
    }
}

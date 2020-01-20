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

//! Service with API, but without any transactions.
//! This service can be used for testing features related only to the API.

use chrono::{TimeZone, Utc};
use exonum::runtime::InstanceId;
use exonum_derive::*;
use exonum_rust_runtime::{
    api::{self, Deprecated, ServiceApiBuilder, ServiceApiState},
    DefaultInstance, Service,
};
use serde_derive::{Deserialize, Serialize};

pub const SERVICE_NAME: &str = "api-service";
pub const SERVICE_ID: InstanceId = 3;

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

    /// Returns `Gone` error.
    fn gone(_state: &ServiceApiState<'_>, _ping: PingQuery) -> api::Result<u64> {
        Err(api::Error::new(api::HttpStatusCode::GONE))
    }

    fn wire(builder: &mut ServiceApiBuilder) {
        let public_scope = builder.public_scope();

        // Normal endpoint.
        public_scope.endpoint("ping-pong", Self::ping_pong);

        // Deprecated endpoints.
        public_scope
            .deprecated_endpoint("ping-pong-deprecated", Deprecated::new(Self::ping_pong))
            .deprecated_endpoint(
                "ping-pong-deprecated-with-deadline",
                Deprecated::new(Self::ping_pong)
                    .with_date(Utc.ymd(2055, 12, 31).and_hms(23, 59, 59)),
            )
            .deprecated_endpoint_mut("ping-pong-deprecated-mut", Deprecated::new(Self::ping_pong));

        // Gone endpoints.
        public_scope
            .endpoint_mut("gone-mutable", Self::gone)
            .endpoint("gone-immutable", Self::gone);

        // Moved endpoints.
        public_scope
            .endpoint_mut(
                "moved-mutable",
                move |state: &ServiceApiState<'_>, _query: PingQuery| -> api::Result<u64> {
                    Err(state.moved_permanently("ping-pong-deprecated-mut").into())
                },
            )
            .endpoint(
                "moved-immutable",
                move |state: &ServiceApiState<'_>, query: PingQuery| -> api::Result<u64> {
                    Err(state
                        .moved_permanently("ping-pong")
                        .with_query(query)
                        .into())
                },
            );

        public_scope.endpoint(
            "error",
            move |_state: &ServiceApiState<'_>, query: PingQuery| -> api::Result<u64> {
                if query.value == 64 {
                    Ok(query.value)
                } else {
                    Err(api::Error::bad_request()
                        .docs_uri("http://some-docs.com")
                        .title("Test endpoint error")
                        .detail(format!("Test endpoint failed with query: {}", query.value))
                        .error_code(42))
                }
            },
        );
    }
}

// // // // Service // // // //

#[derive(Debug, ServiceDispatcher, ServiceFactory)]
#[service_factory(artifact_name = "api-service", artifact_version = "1.0.0")]
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

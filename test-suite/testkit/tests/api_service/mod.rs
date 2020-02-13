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
use exonum::runtime::{
    migrations::{InitMigrationError, MigrateData, MigrationScript},
    versioning::Version,
    InstanceId,
};
use exonum_derive::*;
use exonum_rust_runtime::{
    api::{self, Deprecated, ServiceApiBuilder, ServiceApiState},
    DefaultInstance, Service,
};
use serde_derive::{Deserialize, Serialize};

pub const SERVICE_NAME: &str = "api-service";
// We need supervisor privileges to start service migration.
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

#[derive(Debug, Clone, Copy)]
struct ApiV2;

impl ApiV2 {
    /// Re-envisioned version of `ping-pong` endpoint, designed to have better UX and push
    /// the boundaries of high performance and security.
    fn ping_pong(_state: &ServiceApiState<'_>, ping: PingQuery) -> api::Result<u64> {
        Ok(ping.value + 1)
    }

    fn wire(builder: &mut ServiceApiBuilder) {
        let public_scope = builder.public_scope();
        // Normal endpoint.
        public_scope.endpoint("ping-pong", Self::ping_pong);
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

#[derive(Debug, ServiceDispatcher, ServiceFactory)]
#[service_factory(artifact_name = "api-service", artifact_version = "2.0.0")]
pub struct ApiServiceV2;

impl Service for ApiServiceV2 {
    fn wire_api(&self, builder: &mut ServiceApiBuilder) {
        ApiV2::wire(builder)
    }
}

impl MigrateData for ApiServiceV2 {
    fn migration_scripts(
        &self,
        start_version: &Version,
    ) -> Result<Vec<MigrationScript>, InitMigrationError> {
        if *start_version == Version::new(1, 0, 0) {
            Ok(vec![])
        } else {
            Err(InitMigrationError::NotSupported)
        }
    }
}

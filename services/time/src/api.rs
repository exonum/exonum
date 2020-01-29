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

//! HTTP API for the time service. All APIs are accessible from the public HTTP
//! server of the node.
//!
//! # Public API Endpoints
//!
//! ## Get Current Time
//!
//! | Property    | Value |
//! |-------------|-------|
//! | Path        | `/api/services/{INSTANCE_NAME}/v1/current_time` |
//! | Method      | GET   |
//! | Query type  | - |
//! | Return type | `Option<DateTime<Utc>>` |
//!
//! Returns the current stored time available in `exonum-time` service.
//! `None` will be returned if there is no enough data to provide a trusted
//! time yet.
//!
//! ```
//! # use chrono::{DateTime, Utc};
//! # use exonum::{helpers::Height, runtime::InstanceId};
//! # use exonum_rust_runtime::ServiceFactory;
//! # use exonum_testkit::{ApiKind, TestKit, TestKitBuilder};
//! # use exonum_time::TimeServiceFactory;
//! # const TIME_SERVICE_ID: InstanceId = 100;
//! const TIME_SERVICE_NAME: &'static str = "time-oracle";
//!
//! # fn main() -> Result<(), failure::Error> {
//! let time_service_factory = TimeServiceFactory::default();
//! let time_service_artifact = time_service_factory.artifact_id();
//! let mut testkit: TestKit = TestKitBuilder::validator()
//!     .with_artifact(time_service_artifact.clone())
//!     .with_instance(
//!         time_service_artifact.into_default_instance(TIME_SERVICE_ID, TIME_SERVICE_NAME),
//!     )
//!     .with_rust_service(time_service_factory)
//!     .build();
//! let api = testkit.api();
//!
//! // Make request to the `current_time` endpoint.
//! let response: Option<DateTime<Utc>> = api
//!     .public(ApiKind::Service(TIME_SERVICE_NAME))
//!     .get("v1/current_time")?;
//! // Since no blocks were created yet, time is not available.
//! assert!(response.is_none());
//!
//! // Create some blocks and try again.
//! testkit.create_blocks_until(Height(5));
//! let response: Option<DateTime<Utc>> = api
//!     .public(ApiKind::Service(TIME_SERVICE_NAME))
//!     .get("v1/current_time")?;
//! // At this moment, time should be available.
//! assert!(response.is_some());
//! # Ok(())
//! # }
//! ```
//!
//! # Private API Endpoints
//!
//! ## Get Validators Times
//!
//! | Property    | Value |
//! |-------------|-------|
//! | Path        | `/api/services/{INSTANCE_NAME}/v1/validators_times` |
//! | Method      | GET   |
//! | Query type  | - |
//! | Return type | Vec<[`ValidatorTime`]> |
//!
//! [`ValidatorTime`]: struct.ValidatorTime.html
//!
//! Returns the list of `ValidatorTime` objects for every node that is currently
//! a validator.
//!
//! ```
//! # use chrono::{DateTime, Utc};
//! # use exonum::{helpers::Height, runtime::InstanceId};
//! # use exonum_rust_runtime::ServiceFactory;
//! # use exonum_testkit::{ApiKind, TestKit, TestKitBuilder};
//! # use exonum_time::{TimeServiceFactory, ValidatorTime};
//! # const TIME_SERVICE_ID: InstanceId = 100;
//! const TIME_SERVICE_NAME: &'static str = "time-oracle";
//!
//! # fn main() -> Result<(), failure::Error> {
//! let time_service_factory = TimeServiceFactory::default();
//! let time_service_artifact = time_service_factory.artifact_id();
//! let mut testkit: TestKit = TestKitBuilder::validator()
//!     .with_artifact(time_service_artifact.clone())
//!     .with_instance(
//!         time_service_artifact.into_default_instance(TIME_SERVICE_ID, TIME_SERVICE_NAME),
//!     )
//!     .with_rust_service(time_service_factory)
//!     .build();
//! let api = testkit.api();
//! testkit.create_blocks_until(Height(5));
//!
//! // Obtain validator times.
//! let response: Vec<ValidatorTime> = api
//!     .private(ApiKind::Service(TIME_SERVICE_NAME))
//!     .get("v1/validators_times")?;
//! for validator in response {
//!     assert!(validator.time.is_some());
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ## Get All Times
//!
//! | Property    | Value |
//! |-------------|-------|
//! | Path        | `/api/services/{INSTANCE_NAME}/v1/validators_times/all` |
//! | Method      | GET   |
//! | Query type  | - |
//! | Return type | Vec<[`ValidatorTime`]> |
//!
//! Similar to [`/validator_times`] endpoint, but returns a list of
//! `ValidatorTime` objects for every node that participated in the
//! time consolidating process (some of them may not be validators
//! anymore).
//!
//! [`ValidatorTime`]: struct.ValidatorTime.html
//! [`/validator_times`]: #get-validator-times

use chrono::{DateTime, Utc};
use exonum::crypto::PublicKey;
use exonum_api::Result;
use exonum_rust_runtime::api;
use serde_derive::{Deserialize, Serialize};

use crate::TimeSchema;

/// Structure for saving public key of the validator and last known local time.
#[derive(Debug, Serialize, Deserialize)]
pub struct ValidatorTime {
    /// Public key of the validator.
    pub public_key: PublicKey,
    /// Time of the validator.
    pub time: Option<DateTime<Utc>>,
}

/// Implement the public API for Exonum time.
#[derive(Debug, Clone)]
pub(crate) struct PublicApi;

impl PublicApi {
    /// Endpoint for getting time values for all validators.
    fn current_time(state: &api::ServiceApiState<'_>, _query: ()) -> Result<Option<DateTime<Utc>>> {
        Ok(TimeSchema::new(state.service_data()).time.get())
    }

    /// Extend API.
    pub fn wire(self, builder: &mut api::ServiceApiBuilder) {
        builder
            .public_scope()
            .endpoint("v1/current_time", Self::current_time);
    }
}

/// Implement the private API for Exonum time.
#[derive(Debug, Clone)]
pub(crate) struct PrivateApi;

impl PrivateApi {
    /// Endpoint for getting time values for all validators.
    pub fn all_validators_times(state: &api::ServiceApiState<'_>) -> Result<Vec<ValidatorTime>> {
        let schema = TimeSchema::new(state.service_data());
        // All available times of the validators.
        let validators_times = schema
            .validators_times
            .iter()
            .map(|(public_key, time)| ValidatorTime {
                public_key,
                time: Some(time),
            })
            .collect::<Vec<_>>();
        Ok(validators_times)
    }

    /// Endpoint for getting time values for current validators.
    pub fn current_validators_time(state: &api::ServiceApiState<'_>) -> Result<Vec<ValidatorTime>> {
        let validator_keys = state.data().for_core().consensus_config().validator_keys;
        let schema = TimeSchema::new(state.service_data());

        // Times of the current validators.
        // `None` if the time of the validator is unknown.
        let validators_times = validator_keys
            .iter()
            .map(|validator| ValidatorTime {
                public_key: validator.service_key,
                time: schema.validators_times.get(&validator.service_key),
            })
            .collect::<Vec<_>>();
        Ok(validators_times)
    }

    /// Wires time service API endpoints.
    pub fn wire(self, builder: &mut api::ServiceApiBuilder) {
        builder
            .private_scope()
            .endpoint("v1/validators_times", {
                move |state: &api::ServiceApiState<'_>, _query: ()| {
                    Self::current_validators_time(state)
                }
            })
            .endpoint("v1/validators_times/all", {
                move |state: &api::ServiceApiState<'_>, _query: ()| {
                    Self::all_validators_times(state)
                }
            });
    }
}

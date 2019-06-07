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

//! Exonum-time API.

use chrono::{DateTime, Utc};

use exonum::{api, blockchain::Schema, crypto::PublicKey};

use crate::TimeSchema;

/// Structure for saving public key of the validator and last known local time.
#[derive(Debug, Serialize, Deserialize)]
pub struct ValidatorTime {
    /// Public key of the validator.
    pub public_key: PublicKey,
    /// Time of the validator.
    pub time: Option<DateTime<Utc>>,
}

/// Implements the exonum-time public API.
#[derive(Debug, Clone)]
pub struct PublicApi {
    service_name: String,
}

impl PublicApi {
    /// Creates a new public API instance for service instance with the given name.
    pub fn new(service_name: impl Into<String>) -> Self {
        Self {
            service_name: service_name.into(),
        }
    }

    /// Endpoint for getting time values for all validators.
    pub fn current_time(
        service_name: &str,
        state: &api::ServiceApiState,
    ) -> api::Result<Option<DateTime<Utc>>> {
        let view = state.snapshot();
        let schema = TimeSchema::new(service_name, &view);
        Ok(schema.time().get())
    }

    /// Used to extend Api.
    pub fn wire(self, builder: &mut api::ServiceApiBuilder) {
        let service_name = self.service_name;
        builder.public_scope().endpoint(
            "v1/current_time",
            move |state: &api::ServiceApiState, _query: ()| {
                Self::current_time(&service_name, state)
            },
        );
    }
}

/// Implements the exonum-time private API.
#[derive(Debug, Clone)]
pub struct PrivateApi {
    service_name: String,
}

impl PrivateApi {
    /// Creates a new private API instance for service instance with the given name.
    pub fn new(service_name: impl Into<String>) -> Self {
        Self {
            service_name: service_name.into(),
        }
    }

    /// Endpoint for getting time values for all validators.
    pub fn all_validators_times(
        service_name: &str,
        state: &api::ServiceApiState,
    ) -> api::Result<Vec<ValidatorTime>> {
        let view = state.snapshot();
        let schema = TimeSchema::new(service_name, &view);
        let idx = schema.validators_times();

        // The times of all validators for which time is known.
        let validators_times = idx
            .iter()
            .map(|(public_key, time)| ValidatorTime {
                public_key,
                time: Some(time),
            })
            .collect::<Vec<_>>();
        Ok(validators_times)
    }

    /// Endpoint for getting time values for current validators.
    pub fn current_validators_time(
        service_name: &str,
        state: &api::ServiceApiState,
    ) -> api::Result<Vec<ValidatorTime>> {
        let view = state.snapshot();
        let validator_keys = Schema::new(&view).actual_configuration().validator_keys;
        let schema = TimeSchema::new(service_name, &view);
        let idx = schema.validators_times();

        // The times of current validators.
        // `None` if the time of the validator is unknown.
        let validators_times = validator_keys
            .iter()
            .map(|validator| ValidatorTime {
                public_key: validator.service_key,
                time: idx.get(&validator.service_key),
            })
            .collect::<Vec<_>>();
        Ok(validators_times)
    }

    /// Used to extend Api.
    pub fn wire(self, builder: &mut api::ServiceApiBuilder) {
        builder
            .private_scope()
            .endpoint("v1/validators_times", {
                let service_name = self.service_name.clone();
                move |state: &api::ServiceApiState, _query: ()| {
                    Self::current_validators_time(&service_name, state)
                }
            })
            .endpoint("v1/validators_times/all", {
                let service_name = self.service_name.clone();
                move |state: &api::ServiceApiState, _query: ()| {
                    Self::all_validators_times(&service_name, state)
                }
            });
    }
}

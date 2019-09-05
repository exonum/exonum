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

use exonum::{blockchain::Schema, crypto::PublicKey, runtime::api};

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
pub struct PublicApi;

impl PublicApi {
    /// Endpoint for getting time values for all validators.
    pub fn current_time(
        state: &api::ServiceApiState,
        _query: (),
    ) -> api::Result<Option<DateTime<Utc>>> {
        Ok(TimeSchema::new(state.instance.name, state.snapshot())
            .time()
            .get())
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
pub struct PrivateApi;

impl PrivateApi {
    /// Endpoint for getting time values for all validators.
    pub fn all_validators_times(state: &api::ServiceApiState) -> api::Result<Vec<ValidatorTime>> {
        let view = state.snapshot();
        let schema = TimeSchema::new(state.instance.name, view);
        let idx = schema.validators_times();

        // All available times of the validators.
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
        state: &api::ServiceApiState,
    ) -> api::Result<Vec<ValidatorTime>> {
        let view = state.snapshot();
        let validator_keys = Schema::new(view).actual_configuration().validators;
        let schema = TimeSchema::new(state.instance.name, view);
        let idx = schema.validators_times();

        // Times of the current validators.
        // `None` if the time of the validator is unknown.
        let validators_times = validator_keys
            .iter()
            .map(|validator| ValidatorTime {
                public_key: validator.service,
                time: idx.get(&validator.service),
            })
            .collect::<Vec<_>>();
        Ok(validators_times)
    }

    /// Used to extend Api.
    pub fn wire(self, builder: &mut api::ServiceApiBuilder) {
        builder
            .private_scope()
            .endpoint("v1/validators_times", {
                move |state: &api::ServiceApiState, _query: ()| Self::current_validators_time(state)
            })
            .endpoint("v1/validators_times/all", {
                move |state: &api::ServiceApiState, _query: ()| Self::all_validators_times(state)
            });
    }
}

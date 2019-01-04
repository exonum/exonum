// Copyright 2018 The Exonum Team
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

use TimeSchema;

/// Structure for saving public key of the validator and last known local time.
#[derive(Debug, Serialize, Deserialize)]
pub struct ValidatorTime {
    /// Public key of the validator.
    pub public_key: PublicKey,
    /// Time of the validator.
    pub time: Option<DateTime<Utc>>,
}

/// Implements the exonum-time public API.
#[derive(Debug)]
pub struct PublicApi;

impl PublicApi {
    /// Endpoint for getting time values for all validators.
    pub fn current_time(
        state: &api::ServiceApiState,
        _query: (),
    ) -> api::Result<Option<DateTime<Utc>>> {
        let view = state.snapshot();
        let schema = TimeSchema::new(&view);
        Ok(schema.time().get())
    }

    /// Used to extend Api.
    pub fn wire(builder: &mut api::ServiceApiBuilder) {
        builder
            .public_scope()
            .endpoint("v1/current_time", Self::current_time);
    }
}

/// Implements the exonum-time private API.
#[derive(Debug)]
pub struct PrivateApi;

impl PrivateApi {
    /// Endpoint for getting time values for all validators.
    pub fn all_validators_times(
        state: &api::ServiceApiState,
        _query: (),
    ) -> api::Result<Vec<ValidatorTime>> {
        let view = state.snapshot();
        let schema = TimeSchema::new(&view);
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
        state: &api::ServiceApiState,
        _query: (),
    ) -> api::Result<Vec<ValidatorTime>> {
        let view = state.snapshot();
        let validator_keys = Schema::new(&view).actual_configuration().validator_keys;
        let schema = TimeSchema::new(&view);
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
    pub fn wire(builder: &mut api::ServiceApiBuilder) {
        builder
            .private_scope()
            .endpoint("v1/validators_times", Self::current_validators_time)
            .endpoint("v1/validators_times/all", Self::all_validators_times);
    }
}

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

//! HTTP API interfaces of the time oracle service.

use exonum::api::ext::{ApiError, Endpoint};
use exonum::blockchain::Schema as CoreSchema;
use exonum::crypto::PublicKey;

use std::time::SystemTime;

use super::TimeSchema;

/// Structure for saving public key of the validator and last known local time.
///
/// # JSON presentation
///
/// Represented as an object with two fields:
///
/// ```
/// # extern crate exonum_time;
/// # extern crate serde_json;
/// # use exonum_time::ValidatorTime;
/// # fn main() {
/// let time: ValidatorTime = serde_json::from_str(r#"{
///    "public_key": "83955565ee605f68fe334132b5ae33fe4ae9be2d85fbe0bd9d56734ad4ffdebd",
///    "time": {
///      "nanos_since_epoch": 626107000,
///      "secs_since_epoch": 1516011501
///    }
/// }"#).unwrap();
/// # drop(time);
/// # }
/// ```
#[derive(Debug, Serialize, Deserialize)]
pub struct ValidatorTime {
    /// Public key of the validator.
    pub public_key: PublicKey,
    /// Time of the validator.
    pub time: Option<SystemTime>,
}

read_request! {
    /// Endpoint returning the current consolidated time.
    ///
    /// If there is no consolidated time value yet, returns `None`.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate exonum;
    /// # extern crate exonum_testkit;
    /// # extern crate exonum_time;
    /// #[macro_use] extern crate serde_json;
    /// use exonum_testkit::TestKit;
    /// # use exonum_time::{GetTime, TimeService, TxTime};
    /// use std::time::{self, Duration};
    ///
    /// # fn main() {
    /// let mut testkit = TestKit::for_service(TimeService::new());
    /// // Consolidated time is not set yet
    /// testkit.api().test::<GetTime>(json!(null), &json!(null));
    ///
    /// let tx = {
    ///     let t = time::UNIX_EPOCH + Duration::new(1_400_000_000, 0);
    ///     let (pubkey, key) = testkit.us().service_keypair();
    ///     TxTime::new(t, pubkey, key)
    /// };
    /// testkit.create_block_with_transaction(tx);
    /// // As the network contains only one validator, a single transaction
    /// // is enough to set the time.
    /// testkit.api().test::<GetTime>(
    ///     json!(null),
    ///     &json!({
    ///         "secs_since_epoch": 1400000000,
    ///         "nanos_since_epoch": 0
    ///     }),
    /// );
    /// # }
    /// ```
    @(ID = "current_time")
    pub GetTime(()) -> Option<SystemTime>;
}

impl Endpoint for GetTime {
    fn handle(&self, _: ()) -> Result<Option<SystemTime>, ApiError> {
        Ok(TimeSchema::new(self.as_ref().snapshot()).time().get())
    }
}

read_request! {
    /// Endpoint returning an array of timestamps for the actual validators.
    @(ID = "validators_times")
    pub GetValidatorsTimes(()) -> Vec<ValidatorTime>;
}

impl Endpoint for GetValidatorsTimes {
    fn handle(&self, _: ()) -> Result<Vec<ValidatorTime>, ApiError> {
        let view = self.as_ref().snapshot();
        let validator_keys = CoreSchema::new(&view).actual_configuration().validator_keys;
        let schema = TimeSchema::new(&view);
        let idx = schema.validators_times();

        // The times of current validators.
        // `None` if the time of the validator is unknown.
        let validators_times = validator_keys
            .iter()
            .map(|validator| {
                ValidatorTime {
                    public_key: validator.service_key,
                    time: idx.get(&validator.service_key),
                }
            })
            .collect();
        Ok(validators_times)
    }
}

read_request! {
    /// Endpoint returning an array of current timestamps for actual and past validators.
    @(ID = "all_validators_times")
    pub GetAllTimes(()) -> Vec<ValidatorTime>;
}

impl Endpoint for GetAllTimes {
    fn handle(&self, _: ()) -> Result<Vec<ValidatorTime>, ApiError> {
        let view = self.as_ref().snapshot();
        let schema = TimeSchema::new(&view);
        let idx = schema.validators_times();

        let validators_times = idx.iter()
            .map(|(public_key, time)| {
                ValidatorTime {
                    public_key,
                    time: Some(time),
                }
            })
            .collect();
        Ok(validators_times)
    }
}

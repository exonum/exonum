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

use chrono::{DateTime, Utc};

use exonum::{
    blockchain::ValidatorKeys,
    crypto::{Hash, PublicKey},
};
use exonum_merkledb::{Access, Entry, ObjectHash, ProofMapIndex, RawAccessMut};

const NOT_INITIALIZED: &str = "Time service schema is not initialized";

/// `Exonum-time` service database schema.
#[derive(Debug)]
pub struct TimeSchema<T: Access> {
    /// `DateTime` for every validator. May contain keys corresponding to past validators.
    pub validators_times: ProofMapIndex<T::Base, PublicKey, DateTime<Utc>>,
    /// Consolidated time.
    pub time: Entry<T::Base, DateTime<Utc>>,
}

impl<T: Access> TimeSchema<T> {
    /// Constructs schema for the given `access`.
    pub fn new(access: T) -> Self {
        TimeSchema {
            validators_times: access.proof_map("validators_times").expect(NOT_INITIALIZED),
            time: access.entry("time").expect(NOT_INITIALIZED),
        }
    }

    /// Returns hashes for stored tables.
    pub fn state_hash(&self) -> Vec<Hash> {
        vec![self.validators_times.object_hash(), self.time.object_hash()]
    }
}

impl<T> TimeSchema<T>
where
    T: Access,
    T::Base: RawAccessMut,
{
    pub(crate) fn initialize(access: T) -> Self {
        Self {
            validators_times: access.ensure_proof_map("validators_times"),
            time: access.ensure_entry("time"),
        }
    }

    /// Returns an error if the currently registered validator time is greater than `time`.
    pub(crate) fn update_validator_time(
        &mut self,
        author: PublicKey,
        time: DateTime<Utc>,
    ) -> Result<(), ()> {
        match self.validators_times.get(&author) {
            // The validator time in the storage should be less than in the transaction.
            Some(val_time) if val_time >= time => Err(()),
            // Write the time for the validator.
            _ => {
                self.validators_times.put(&author, time);
                Ok(())
            }
        }
    }

    pub(crate) fn update_consolidated_time(&mut self, validator_keys: &[ValidatorKeys]) {
        // Find all known times for the validators.
        let validator_times = {
            let mut times = self
                .validators_times
                .iter()
                .filter_map(|(public_key, time)| {
                    validator_keys
                        .iter()
                        .find(|validator| validator.service_key == public_key)
                        .map(|_| time)
                })
                .collect::<Vec<_>>();
            // Ordering time from highest to lowest.
            times.sort_by(|a, b| b.cmp(a));
            times
        };

        // The largest number of Byzantine nodes.
        let max_byzantine_nodes = (validator_keys.len() - 1) / 3;
        if validator_times.len() <= 2 * max_byzantine_nodes {
            return;
        }

        match self.time.get() {
            // Selected time should be greater than the time in the storage.
            Some(current_time) if current_time >= validator_times[max_byzantine_nodes] => {}
            _ => {
                // Change the time in the storage.
                self.time.set(validator_times[max_byzantine_nodes]);
            }
        }
    }
}

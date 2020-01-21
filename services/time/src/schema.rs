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

use chrono::{DateTime, Utc};

use exonum::{blockchain::ValidatorKeys, crypto::PublicKey};
use exonum_derive::*;
use exonum_merkledb::{
    access::{Access, FromAccess, RawAccessMut},
    ProofEntry, ProofMapIndex,
};

/// Database schema of the time service. The schema is fully public.
#[derive(Debug, FromAccess, RequireArtifact)]
pub struct TimeSchema<T: Access> {
    /// `DateTime` for every validator. May contain keys corresponding to past validators.
    pub validators_times: ProofMapIndex<T::Base, PublicKey, DateTime<Utc>>,
    /// Consolidated blockchain time, approved by validators.
    pub time: ProofEntry<T::Base, DateTime<Utc>>,
}

impl<T: Access> TimeSchema<T> {
    pub(crate) fn new(access: T) -> Self {
        Self::from_root(access).unwrap()
    }
}

impl<T: Access> TimeSchema<T>
where
    T::Base: RawAccessMut,
{
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

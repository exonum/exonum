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
    blockchain::{ExecutionError, ExecutionResult, Schema},
    crypto::PublicKey,
};
use exonum_merkledb::{Fork, Snapshot};

use crate::schema::TimeSchema;

use super::proto;

/// Common errors emitted by transactions during execution.
#[derive(Debug, Fail)]
#[repr(u8)]
pub enum Error {
    /// The sender of the transaction is not among the active validators.
    #[fail(display = "Not authored by a validator")]
    UnknownSender = 0,

    /// The validator time that is stored in storage is greater than the proposed one.
    #[fail(display = "The validator time is greater than the proposed one")]
    ValidatorTimeIsGreater = 1,
}

impl From<Error> for ExecutionError {
    fn from(value: Error) -> ExecutionError {
        let description = value.to_string();
        ExecutionError::with_description(value as u8, description)
    }
}

/// Transaction that is sent by the validator after the commit of the block.
#[derive(Serialize, Deserialize, Debug, Clone, ProtobufConvert)]
#[exonum(pb = "proto::TxTime")]
pub struct TxTime {
    /// Time of the validator.
    pub time: DateTime<Utc>,
}

impl TxTime {
    /// New TxTime transaction.
    pub fn new(time: DateTime<Utc>) -> Self {
        Self { time }
    }
}

impl TxTime {
    pub(crate) fn check_signed_by_validator(
        &self,
        snapshot: &dyn Snapshot,
        author: &PublicKey,
    ) -> ExecutionResult {
        let keys = Schema::new(snapshot).actual_configuration().validator_keys;
        let signed = keys.iter().any(|k| k.service_key == *author);
        if !signed {
            Err(Error::UnknownSender)?
        } else {
            Ok(())
        }
    }

    pub(crate) fn update_validator_time(
        &self,
        service_name: &str,
        fork: &Fork,
        author: &PublicKey,
    ) -> ExecutionResult {
        let schema = TimeSchema::new(service_name, fork);
        let mut validators_times = schema.validators_times();
        match validators_times.get(author) {
            // The validator time in the storage should be less than in the transaction.
            Some(time) if time >= self.time => Err(Error::ValidatorTimeIsGreater)?,
            // Write the time for the validator.
            _ => {
                validators_times.put(author, self.time);
                Ok(())
            }
        }
    }

    pub(crate) fn update_consolidated_time(service_name: &str, fork: &Fork) {
        let keys = Schema::new(fork).actual_configuration().validator_keys;
        let schema = TimeSchema::new(service_name, fork);

        // Find all known times for the validators.
        let validator_times = {
            let idx = schema.validators_times();
            let mut times = idx
                .iter()
                .filter_map(|(public_key, time)| {
                    keys.iter()
                        .find(|validator| validator.service_key == public_key)
                        .map(|_| time)
                })
                .collect::<Vec<_>>();
            // Ordering time from highest to lowest.
            times.sort_by(|a, b| b.cmp(a));
            times
        };

        // The largest number of Byzantine nodes.
        let max_byzantine_nodes = (keys.len() - 1) / 3;
        if validator_times.len() <= 2 * max_byzantine_nodes {
            return;
        }

        let mut time = schema.time();
        match time.get() {
            // Selected time should be greater than the time in the storage.
            Some(current_time) if current_time >= validator_times[max_byzantine_nodes] => {
                return;
            }
            _ => {
                // Change the time in the storage.
                time.set(validator_times[max_byzantine_nodes]);
            }
        }
    }
}

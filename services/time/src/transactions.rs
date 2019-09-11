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
use exonum::{blockchain::Schema, crypto::PublicKey, runtime::rust::TransactionContext};
use exonum_merkledb::IndexAccess;

use crate::{proto, schema::TimeSchema, TimeService};

/// Common errors emitted by transactions during execution.
#[derive(Debug, IntoExecutionError)]
pub enum Error {
    /// The sender of the transaction is not among the active validators.
    UnknownSender = 0,
    /// The validator time that is stored in storage is greater than the proposed one.
    ValidatorTimeIsGreater = 1,
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
        snapshot: impl IndexAccess,
        author: &PublicKey,
    ) -> Result<(), Error> {
        let keys = Schema::new(snapshot).consensus_config().validator_keys;
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
        fork: impl IndexAccess,
        author: &PublicKey,
    ) -> Result<(), Error> {
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

    pub(crate) fn update_consolidated_time(service_name: &str, access: impl IndexAccess) {
        let keys = Schema::new(access.clone())
            .consensus_config()
            .validator_keys;
        let schema = TimeSchema::new(service_name, access);

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
            Some(current_time) if current_time >= validator_times[max_byzantine_nodes] => {}
            _ => {
                // Change the time in the storage.
                time.set(validator_times[max_byzantine_nodes]);
            }
        }
    }
}

/// Time oracle service transaction.
#[exonum_service]
pub trait TimeOracleInterface {
    /// Receives a new time from one of validators.
    fn time(&self, ctx: TransactionContext, arg: TxTime) -> Result<(), Error>;
}

impl TimeOracleInterface for TimeService {
    fn time(&self, context: TransactionContext, arg: TxTime) -> Result<(), Error> {
        let author = context
            .caller()
            .as_transaction()
            .expect("Wrong `TxTime` initiator")
            .1;

        arg.check_signed_by_validator(context.fork(), &author)?;
        arg.update_validator_time(context.instance.name, context.fork(), &author)?;
        TxTime::update_consolidated_time(context.instance.name, context.fork());
        Ok(())
    }
}

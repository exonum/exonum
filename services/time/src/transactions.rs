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
use exonum::runtime::{rust::CallContext, ExecutionError};
use exonum_proto::ProtobufConvert;

use crate::{proto, schema::TimeSchema, TimeService};

/// Common errors emitted by transactions during execution.
#[derive(Debug, ServiceFail)]
pub enum Error {
    /// The sender of the transaction is not among the active validators.
    UnknownSender = 0,
    /// The validator time that is stored in storage is greater than the proposed one.
    ValidatorTimeIsGreater = 1,
}

/// Transaction that is sent by the validator after the commit of the block.
#[derive(Serialize, Deserialize, Debug, Clone, ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "proto::TxTime")]
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

/// Time oracle service transaction.
#[exonum_interface]
pub trait TimeOracleInterface {
    /// Receives a new time from one of validators.
    fn time(&self, ctx: CallContext<'_>, arg: TxTime) -> Result<(), ExecutionError>;
}

impl TimeOracleInterface for TimeService {
    fn time(&self, context: CallContext<'_>, arg: TxTime) -> Result<(), ExecutionError> {
        let author = context
            .caller()
            .author()
            .ok_or_else(|| context.err(Error::UnknownSender))?;
        // Check that the transaction is signed by a validator.
        let core_schema = context.data().for_core();
        core_schema
            .validator_id(author)
            .ok_or_else(|| context.err(Error::UnknownSender))?;

        let mut schema = TimeSchema::new(context.service_data());
        schema
            .update_validator_time(author, arg.time)
            .map_err(|()| context.err(Error::ValidatorTimeIsGreater))?;

        let validator_keys = core_schema.consensus_config().validator_keys;
        schema.update_consolidated_time(&validator_keys);
        Ok(())
    }
}

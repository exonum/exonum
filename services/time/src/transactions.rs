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
use exonum::runtime::{CommonError, ExecutionContext, ExecutionError};
use exonum_derive::{exonum_interface, interface_method, BinaryValue, ExecutionFail, ObjectHash};
use exonum_proto::ProtobufConvert;
use serde::{Deserialize, Serialize};

use crate::{proto, schema::TimeSchema, TimeService};

/// Common errors emitted by transactions during execution.
#[derive(Debug, ExecutionFail)]
pub enum Error {
    /// The validator time that is stored in storage is greater than the proposed one.
    ValidatorTimeIsGreater = 0,
}

/// Transaction that is sent by the validator after the commit of the block.
#[derive(Clone, Debug)]
#[derive(Serialize, Deserialize)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash)]
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
pub trait TimeOracleInterface<Ctx> {
    /// Output of the methods in this interface.
    type Output;

    /// Receives a new time from one of validators.
    ///
    /// Transaction sent not by a validator will be discarded.
    #[interface_method(id = 0)]
    fn report_time(&self, ctx: Ctx, arg: TxTime) -> Self::Output;
}

impl TimeOracleInterface<ExecutionContext<'_>> for TimeService {
    type Output = Result<(), ExecutionError>;

    fn report_time(&self, context: ExecutionContext<'_>, arg: TxTime) -> Self::Output {
        let author = context
            .caller()
            .author()
            .ok_or(CommonError::UnauthorizedCaller)?;
        // Check that the transaction is signed by a validator.
        let core_schema = context.data().for_core();
        core_schema
            .validator_id(author)
            .ok_or(CommonError::UnauthorizedCaller)?;

        let mut schema = TimeSchema::new(context.service_data());
        schema
            .update_validator_time(author, arg.time)
            .map_err(|()| Error::ValidatorTimeIsGreater)?;

        let validator_keys = core_schema.consensus_config().validator_keys;
        schema.update_consolidated_time(&validator_keys);
        Ok(())
    }
}

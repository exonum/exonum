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

//! Timestamping transactions.

use exonum::{
    blockchain::{ExecutionError, ExecutionResult},
    messages::ServiceInstanceId,
    runtime::rust::TransactionContext,
};
use exonum_time::schema::TimeSchema;

use crate::{
    proto,
    schema::{Schema, Timestamp},
    TimestampEntry, TimestampingService,
};

/// Error codes emitted by wallet transactions during execution.
#[derive(Debug, Fail)]
#[repr(u8)]
pub enum Error {
    /// Content hash already exists.
    #[fail(display = "Content hash already exists")]
    HashAlreadyExists = 0,
}

impl From<Error> for ExecutionError {
    fn from(value: Error) -> ExecutionError {
        let description = value.to_string();
        ExecutionError::with_description(value as u8, description)
    }
}

/// Timestamping transaction.
#[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert)]
#[exonum(pb = "proto::TxTimestamp")]
pub struct TxTimestamp {
    /// Timestamp content.
    pub content: Timestamp,
}

/// Timestamping configuration parameters.
#[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert)]
#[exonum(pb = "proto::Configuration")]
pub struct Configuration {
    /// Time oracle service name.
    pub time_service_name: String,
    /// Time oracle service id.
    pub time_service_id: ServiceInstanceId,
}

#[service_interface]
pub trait TimestampingInterface {
    fn timestamp(&self, ctx: TransactionContext, arg: TxTimestamp) -> ExecutionResult;
}

impl TimestampingInterface for TimestampingService {
    fn timestamp(&self, context: TransactionContext, arg: TxTimestamp) -> ExecutionResult {
        let tx_hash = context.tx_hash();

        let schema = Schema::new(context.service_name(), context.fork());

        let config = schema.config().get().expect("Can't read service config");

        let time = TimeSchema::new(&config.time_service_name, context.fork())
            .time()
            .get()
            .expect("Can't get the time");

        let hash = &arg.content.content_hash;

        if let Some(_entry) = schema.timestamps().get(hash) {
            Err(Error::HashAlreadyExists)?;
        }

        trace!("Timestamp added: {:?}", arg);
        let entry = TimestampEntry::new(arg.content.clone(), &tx_hash, time);
        schema.add_timestamp(entry);

        Ok(())
    }
}

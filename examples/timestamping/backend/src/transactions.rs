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

use exonum::runtime::{rust::CallContext, DispatcherError, ExecutionError};
use exonum_proto::ProtobufConvert;
use exonum_time::schema::TimeSchema;

use crate::{
    proto,
    schema::{Schema, Timestamp},
    TimestampEntry, TimestampingService,
};

/// Error codes emitted by wallet transactions during execution.
#[derive(Debug, ServiceFail)]
pub enum Error {
    /// Content hash already exists.
    HashAlreadyExists = 0,
    /// Time service with the specified name doesn't exist.
    TimeServiceNotFound = 1,
}

/// Timestamping transaction.
#[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "proto::TxTimestamp")]
pub struct TxTimestamp {
    /// Timestamp content.
    pub content: Timestamp,
}

/// Timestamping configuration parameters.
#[derive(Serialize, Deserialize, Clone, Debug, ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "proto::Config")]
pub struct Config {
    /// Time oracle service name.
    pub time_service_name: String,
}

#[exonum_interface]
pub trait TimestampingInterface {
    fn timestamp(&self, ctx: CallContext<'_>, arg: TxTimestamp) -> Result<(), ExecutionError>;
}

impl TimestampingInterface for TimestampingService {
    fn timestamp(&self, context: CallContext<'_>, arg: TxTimestamp) -> Result<(), ExecutionError> {
        let (tx_hash, _) = context
            .caller()
            .as_transaction()
            .ok_or(DispatcherError::UnauthorizedCaller)?;

        let mut schema = Schema::new(context.service_data());
        let config = schema.config.get().expect("Can't read service config");

        let data = context.data();
        let time_service_data = data
            .for_service(config.time_service_name.as_str())
            .ok_or_else(|| context.err(Error::TimeServiceNotFound))?;
        let time = TimeSchema::new(time_service_data)
            .time
            .get()
            .ok_or_else(|| context.err(Error::TimeServiceNotFound))?;

        let hash = &arg.content.content_hash;
        if schema.timestamps.get(hash).is_some() {
            Err(context.err(Error::HashAlreadyExists))
        } else {
            trace!("Timestamp added: {:?}", arg);
            let entry = TimestampEntry::new(arg.content.clone(), tx_hash, time);
            schema.add_timestamp(entry);
            Ok(())
        }
    }
}

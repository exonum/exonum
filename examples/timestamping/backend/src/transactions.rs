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
use exonum_derive::{exonum_interface, BinaryValue, ExecutionFail, ObjectHash};
use exonum_proto::ProtobufConvert;
use exonum_time::schema::TimeSchema;
use log::trace;

use crate::{
    proto,
    schema::{Schema, Timestamp},
    TimestampEntry, TimestampingService,
};

/// Error codes emitted by wallet transactions during execution.
#[derive(Debug, ExecutionFail)]
pub enum Error {
    /// Content hash already exists.
    HashAlreadyExists = 0,
    /// Time service with the specified name doesn't exist.
    TimeServiceNotFound = 1,
}

/// Timestamping transaction.
#[derive(Clone, Debug)]
#[derive(Serialize, Deserialize)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "proto::TxTimestamp")]
pub struct TxTimestamp {
    /// Timestamp content.
    pub content: Timestamp,
}

/// Timestamping configuration parameters.
#[derive(Clone, Debug)]
#[derive(Serialize, Deserialize)]
#[derive(ProtobufConvert, BinaryValue, ObjectHash)]
#[protobuf_convert(source = "proto::Config")]
pub struct Config {
    /// Time oracle service name.
    pub time_service_name: String,
}

#[exonum_interface]
pub trait TimestampingInterface<Ctx> {
    type Output;
    fn timestamp(&self, ctx: Ctx, arg: TxTimestamp) -> Self::Output;
}

impl TimestampingInterface<CallContext<'_>> for TimestampingService {
    type Output = Result<(), ExecutionError>;

    fn timestamp(&self, context: CallContext<'_>, arg: TxTimestamp) -> Self::Output {
        let (tx_hash, _) = context
            .caller()
            .as_transaction()
            .ok_or(DispatcherError::UnauthorizedCaller)?;

        let mut schema = Schema::new(context.service_data());
        let config = schema.config.get().expect("Can't read service config");

        let data = context.data();
        let time_service_data = data
            .for_service(config.time_service_name.as_str())
            .ok_or(Error::TimeServiceNotFound)?;
        let time = TimeSchema::new(time_service_data)
            .time
            .get()
            .ok_or(Error::TimeServiceNotFound)?;

        let hash = &arg.content.content_hash;
        if schema.timestamps.get(hash).is_some() {
            Err(Error::HashAlreadyExists.into())
        } else {
            trace!("Timestamp added: {:?}", arg);
            let entry = TimestampEntry::new(arg.content.clone(), tx_hash, time);
            schema.add_timestamp(entry);
            Ok(())
        }
    }
}

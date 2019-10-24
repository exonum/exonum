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

use exonum::runtime::rust::TransactionContext;
use exonum_proto::ProtobufConvert;
use exonum_time::schema::TimeSchema;

use crate::{
    proto,
    schema::{Schema, Timestamp},
    TimestampEntry, TimestampingService,
};

/// Error codes emitted by wallet transactions during execution.
#[derive(Debug, IntoExecutionError)]
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

#[exonum_service]
pub trait TimestampingInterface {
    fn timestamp(&self, ctx: TransactionContext, arg: TxTimestamp) -> Result<(), Error>;
}

impl TimestampingInterface for TimestampingService {
    fn timestamp(&self, context: TransactionContext, arg: TxTimestamp) -> Result<(), Error> {
        let tx_hash = context
            .caller()
            .as_transaction()
            .expect("Wrong `TxTimestamp` initiator")
            .0;

        let schema = Schema::new(context.instance.name, context.fork());

        let config = schema.config().get().expect("Can't read service config");

        let time = TimeSchema::new(&config.time_service_name, context.fork())
            .time()
            .get()
            .expect("Can't get the time");

        let hash = &arg.content.content_hash;

        if schema.timestamps().get(hash).is_some() {
            Err(Error::HashAlreadyExists)
        } else {
            trace!("Timestamp added: {:?}", arg);
            let entry = TimestampEntry::new(arg.content.clone(), tx_hash, time);
            schema.add_timestamp(entry);
            Ok(())
        }
    }
}

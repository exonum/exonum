// Copyright 2018 The Exonum Team
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

// Workaround for `failure` see https://github.com/rust-lang-nursery/failure/issues/223 and
// ECR-1771 for the details.
#![allow(bare_trait_objects)]
// Suppress a warning in `transactions!` macro call:
#![cfg_attr(feature = "cargo-clippy", allow(redundant_field_names))]

use exonum::{
    blockchain::{ExecutionError, ExecutionResult, Transaction, TransactionContext},
    crypto::{PublicKey, SecretKey}, messages::{Message, Protocol, RawTransaction},
};
use exonum_time::schema::TimeSchema;

use schema::{Schema, Timestamp, TimestampEntry};
use TIMESTAMPING_SERVICE;

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

transactions! {
    /// Transaction group.
    pub TimeTransactions {

        /// A timestamp transaction.
        struct TxTimestamp {
            /// Timestamp content.
            content: Timestamp,
        }
    }
}

impl TxTimestamp {
    #[doc(hidden)]
    pub fn sign(
        author: &PublicKey,
        content: Timestamp,
        key: &SecretKey,
    ) -> Message<RawTransaction> {
        Protocol::sign_transaction(
            TxTimestamp::new(content),
            TIMESTAMPING_SERVICE,
            *author,
            key,
        )
    }
}

impl Transaction for TxTimestamp {
    fn execute(&self, mut context: TransactionContext) -> ExecutionResult {
        let tx_hash = context.tx_hash();
        let time = TimeSchema::new(&context.fork())
            .time()
            .get()
            .expect("Can't get the time");

        let content = self.content();
        let hash = content.content_hash();

        let mut schema = Schema::new(context.fork());
        if let Some(_entry) = schema.timestamps().get(hash) {
            Err(Error::HashAlreadyExists)?;
        }

        trace!("Timestamp added: {:?}", self);
        let entry = TimestampEntry::new(self.content(), &tx_hash, time);
        schema.add_timestamp(entry);
        Ok(())
    }
}
